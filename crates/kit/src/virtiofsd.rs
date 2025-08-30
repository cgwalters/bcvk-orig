use std::process::{Child, Command};

use color_eyre::eyre::Context;
use color_eyre::Result;

#[derive(Debug)]
pub struct VirtiofsdConfig {
    pub socket_path: String,
    pub shared_dir: String,
    pub cache_mode: String,
    pub sandbox: String,
}

impl Default for VirtiofsdConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/inner-shared/virtiofs.sock".to_string(),
            shared_dir: "/run/source-image".to_string(),
            cache_mode: "always".to_string(),
            sandbox: "none".to_string(),
        }
    }
}

pub fn spawn_virtiofsd(config: &VirtiofsdConfig) -> Result<Child> {
    Command::new("/usr/libexec/virtiofsd")
        .args([
            "--socket-path",
            &config.socket_path,
            "--shared-dir",
            &config.shared_dir,
            "--cache",
            &config.cache_mode,
            "--sandbox",
            &config.sandbox,
        ])
        .spawn()
        .context("Failed to spawn virtiofsd")
}
