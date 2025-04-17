//! Environment detection for containerized and host environments
//!
//! Detects container environments, privilege levels (--privileged, --pid=host),
//! and extracts container metadata. Results are cached for performance.
//!

use std::{
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, OnceLock},
};

use color_eyre::{eyre::Context, Result};

use cap_std_ext::cap_std::{self, fs::Dir};
use serde::{Deserialize, Serialize};

/// Environment detection results
///
/// Contains detected container state, privilege levels, and optional
/// container metadata. All fields are determined once and cached.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Environment {
    /// Container run with --privileged
    pub privileged: bool,
    /// Container run with --pid=host  
    pub pidhost: bool,
    /// Running in container (detected via /run/.containerenv)
    pub container: bool,
    /// Parsed container execution info
    pub containerenv: Option<super::containerenv::ContainerExecutionInfo>,
}

/// Detect if running with host PID namespace access (--pid=host)
///
/// Checks for UID differences between parent and current process, or
/// compares mount namespaces if UIDs are the same.
fn is_hostpid() -> Result<bool> {
    let Some(ppid) = rustix::process::getppid() else {
        return Ok(false);
    };
    let myuid = rustix::process::getuid();
    let parent_proc = format!("/proc/{}", ppid.as_raw_nonzero());
    let parent_st = Path::new(&parent_proc).metadata()?;
    // If the parent has a different uid, that's a strong signal we're
    // running with a uid mapping but we can see our real parent in the
    // host pidns.
    if parent_st.uid() != myuid.as_raw() {
        return Ok(true);
    }
    let parent_rootns = std::fs::read_link(format!("/proc/{}/ns/mnt", ppid.as_raw_nonzero()))
        .context("Reading parent mountns")?;
    let my_rootns = std::fs::read_link("/proc/self/ns/mnt").context("Reading self mountns")?;
    Ok(parent_rootns != my_rootns)
}

/// Get cached root filesystem directory handle
///
/// Returns thread-safe cached `Arc<Dir>` for root filesystem access.
/// Uses `OnceLock` for lazy initialization and concurrent safety.
pub(crate) fn global_rootfs(authority: cap_std::AmbientAuthority) -> Result<Arc<Dir>> {
    static ROOTFS: OnceLock<Arc<Dir>> = OnceLock::new();
    if let Some(r) = ROOTFS.get() {
        return Ok(r.clone());
    }
    let r = Dir::open_ambient_dir("/", authority)?;
    let _ = ROOTFS.set(Arc::new(r));
    Ok(ROOTFS.get().unwrap().clone())
}

impl Environment {
    /// Detect current execution environment
    ///
    /// Performs privilege detection, container detection, and namespace analysis.
    /// Designed to handle partial failures gracefully.
    fn new() -> Result<Self> {
        let rootfs = &global_rootfs(cap_std::ambient_authority())?;
        let privileged =
            rustix::thread::capability_is_in_bounding_set(rustix::thread::Capability::SystemAdmin)?;
        let container = super::containerenv::is_container(&rootfs)?;
        let containerenv = super::containerenv::get_container_execution_info(&rootfs)?;
        let pidhost = is_hostpid()?;
        Ok(Environment {
            privileged,
            pidhost,
            containerenv,
            container,
        })
    }

    /// Get cached Environment instance
    ///
    /// Performs detection once per process lifetime and caches results.
    /// Thread-safe with `OnceLock`. Retries on failure until successful.
    pub fn get_cached() -> Result<&'static Self> {
        static INFO: std::sync::OnceLock<Environment> = std::sync::OnceLock::new();
        if let Some(r) = INFO.get() {
            return Ok(r);
        }
        let r = Self::new()?;
        // Discard duplicate initialization attempts from concurrent threads
        let _ = INFO.set(r);
        // SAFETY: We confirmed initialization occurred above
        Ok(INFO.get().unwrap())
    }
}
