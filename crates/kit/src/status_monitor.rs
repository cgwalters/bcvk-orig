use color_eyre::Result;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use tracing::{debug, warn};

use crate::supervisor_status::SupervisorStatus;

/// Monitor a status file for changes using inotify
pub fn monitor_status_file<P: AsRef<Path>>(
    path: P,
    timeout: Duration,
) -> Result<impl Iterator<Item = Result<SupervisorStatus>>> {
    let path = path.as_ref();
    let parent_dir = path.parent().unwrap_or(Path::new("/"));

    debug!("Setting up file watcher for: {}", path.display());

    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )?;

    // Watch the parent directory since the file might not exist yet
    watcher.watch(parent_dir, RecursiveMode::NonRecursive)?;

    Ok(StatusFileIterator {
        path: path.to_path_buf(),
        receiver: rx,
        _watcher: watcher,
        timeout,
        last_mtime: None,
    })
}

struct StatusFileIterator {
    path: std::path::PathBuf,
    receiver: Receiver<notify::Result<notify::Event>>,
    _watcher: RecommendedWatcher,
    timeout: Duration,
    last_mtime: Option<std::time::SystemTime>,
}

impl Iterator for StatusFileIterator {
    type Item = Result<SupervisorStatus>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // First, try to read the file if it exists and has changed
            if let Some(status) = self.try_read_status_if_changed() {
                return Some(status);
            }

            // Wait for file system events with timeout
            let event = self.receiver.recv_timeout(self.timeout).ok()?.ok()?;
            // Check if this event is for our target file
            if self.is_relevant_event(&event) {
                if let Some(status) = self.try_read_status_if_changed() {
                    return Some(status);
                }
            }
        }
    }
}

impl StatusFileIterator {
    fn try_read_status_if_changed(&mut self) -> Option<Result<SupervisorStatus>> {
        // Check if file exists and get its mtime
        let metadata = match std::fs::metadata(&self.path) {
            Ok(meta) => meta,
            Err(_) => return None, // File doesn't exist yet
        };

        let current_mtime = metadata.modified().ok()?;

        // Check if mtime has changed
        let mtime_changed = match self.last_mtime {
            None => true, // First time reading
            Some(last) => current_mtime != last,
        };

        if !mtime_changed {
            return None; // No change, don't emit
        }

        // Update our tracked mtime
        self.last_mtime = Some(current_mtime);

        // Read and return the status
        Some(SupervisorStatus::read_from_file(&self.path))
    }

    fn is_relevant_event(&self, event: &notify::Event) -> bool {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                event.paths.iter().any(|p| p == &self.path)
            }
            _ => false,
        }
    }
}

/// Monitor status and stream updates to stdout as JSON lines
pub fn monitor_and_stream_status() -> Result<()> {
    let path = "/run/supervisor-status.json";
    let timeout = Duration::from_secs(60); // Default timeout

    let monitor = monitor_status_file(path, timeout)?;

    for status_result in monitor {
        match status_result {
            Ok(status) => {
                // Output as JSON line - just stream every update
                if let Ok(json) = serde_json::to_string(&status) {
                    println!("{}", json);
                    // Flush stdout to ensure immediate delivery
                    use std::io::Write;
                    std::io::stdout().flush().unwrap_or(());
                }
            }
            Err(e) => {
                warn!("Error reading status: {}", e);
            }
        }
    }

    Ok(())
}
