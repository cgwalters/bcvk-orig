use color_eyre::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tracing::debug;

use crate::run_ephemeral::{run_detached, RunEphemeralOpts};
use crate::ssh;
use std::path::Path;

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
pub struct RunEphemeralSshOpts {
    #[command(flatten)]
    pub run_opts: RunEphemeralOpts,

    /// SSH command to execute (optional, defaults to interactive shell)
    #[arg(trailing_var_arg = true)]
    pub ssh_args: Vec<String>,
}

/// Wait for systemd to report READY=1 in the guest notification file
///
/// Monitors /run/systemd-guest.txt inside the container for the READY=1 message
/// that indicates the VM has fully booted and is ready for connections.
pub fn wait_for_systemd_ready(container_name: &str, timeout: Duration) -> Result<()> {
    debug!(
        "Waiting for systemd READY=1 notification (timeout: {}s)...",
        timeout.as_secs()
    );
    let start_time = Instant::now();

    while start_time.elapsed() < timeout {
        let status = Command::new("podman")
            .args([
                "exec",
                container_name,
                "grep",
                "-q",
                "READY=1",
                "/run/systemd-guest.txt",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if let Ok(status) = status {
            if status.success() {
                debug!("Received systemd READY=1 notification");
                return Ok(());
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(color_eyre::eyre::eyre!(
        "Timeout waiting for systemd READY=1 notification after {}s",
        timeout.as_secs()
    ))
}

/// Run an ephemeral pod and immediately SSH into it, with lifecycle binding
pub fn run_ephemeral_ssh(opts: RunEphemeralSshOpts) -> Result<()> {
    // Start the ephemeral pod in detached mode with SSH enabled
    let mut ephemeral_opts = opts.run_opts.clone();
    ephemeral_opts.podman.rm = true;
    ephemeral_opts.podman.detach = true;
    ephemeral_opts.common.ssh_keygen = true; // Enable SSH key generation and access

    debug!("Starting ephemeral VM...");
    let container_id = run_detached(ephemeral_opts)?;
    debug!("Ephemeral VM started with container ID: {}", container_id);

    // Use the container ID for SSH and cleanup
    let container_name = container_id;
    debug!("Using container ID: {}", container_name);

    // Wait for systemd to signal readiness instead of arbitrary sleep
    wait_for_systemd_ready(&container_name, Duration::from_secs(60))?;

    // Execute SSH connection directly (no thread needed for this)
    // This allows SSH output to be properly forwarded to stdout/stderr
    debug!("Connecting to SSH...");
    let status = ssh::connect_via_container_with_status(
        &container_name,
        Path::new("/tmp/ssh"),
        "root",
        opts.ssh_args,
    )?;
    debug!("SSH connection completed");

    let exit_code = status.code().unwrap_or(1);
    debug!("SSH exit code: {}", exit_code);

    // SSH completed, proceed with cleanup

    // Cleanup: stop and remove the container immediately
    debug!("SSH session ended, cleaning up ephemeral pod...");

    let _ = Command::new("podman")
        .args(["rm", "-f", &container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Exit with SSH client's exit code
    std::process::exit(exit_code);
}
