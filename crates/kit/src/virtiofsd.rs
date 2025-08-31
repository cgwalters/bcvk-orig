use std::process::{Child, Command, Stdio};

use color_eyre::eyre::Context;
use color_eyre::Result;

#[derive(Debug)]
pub struct VirtiofsdConfig {
    pub socket_path: String,
    pub shared_dir: String,
    pub cache_mode: String,
    pub sandbox: String,
    pub debug: bool,
}

impl Default for VirtiofsdConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/inner-shared/virtiofs.sock".to_string(),
            shared_dir: "/run/source-image".to_string(),
            cache_mode: "always".to_string(),
            sandbox: "none".to_string(),
            debug: false,
        }
    }
}

pub fn spawn_virtiofsd(config: &VirtiofsdConfig) -> Result<Child> {
    let mut cmd = Command::new("/usr/libexec/virtiofsd");
    cmd.args([
        "--socket-path",
        &config.socket_path,
        "--shared-dir",
        &config.shared_dir,
        "--cache",
        &config.cache_mode,
        "--sandbox",
        &config.sandbox,
    ]);

    // Redirect stdout/stderr to /dev/null unless debug mode is enabled
    if !config.debug {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    cmd.spawn().context("Failed to spawn virtiofsd")
}
