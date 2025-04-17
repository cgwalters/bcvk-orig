use color_eyre::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tracing::debug;

use crate::run_ephemeral::{run_detached, RunEphemeralOpts};
use crate::ssh;

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
/// Returns Ok(true) if READY=1 was found, Ok(false) if the notification file doesn't exist
/// (indicating systemd < 254), or an error on timeout.
pub fn wait_for_systemd_ready(container_name: &str, timeout: Duration) -> Result<bool> {
    debug!(
        "Checking for systemd notification support (timeout: {}s)...",
        timeout.as_secs()
    );

    // First check if the notification file exists
    let check_file = Command::new("podman")
        .args([
            "exec",
            container_name,
            "test",
            "-f",
            "/run/systemd-guest.txt",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Ok(status) = check_file {
        if !status.success() {
            debug!("systemd notification file doesn't exist (systemd < 254), skipping notification wait");
            return Ok(false);
        }
    }

    debug!("systemd notification file exists, waiting for READY=1...");
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
                return Ok(true);
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(color_eyre::eyre::eyre!(
        "Timeout waiting for systemd READY=1 notification after {}s",
        timeout.as_secs()
    ))
}

/// Wait for SSH to be ready by polling SSH connection attempts
///
/// Attempts to connect to the VM via SSH until successful or timeout.
/// This is used as a fallback when systemd notification is not available.
pub fn wait_for_ssh_ready(container_name: &str, timeout: Duration) -> Result<()> {
    debug!(
        "Polling SSH connectivity (timeout: {}s)...",
        timeout.as_secs()
    );
    let start_time = Instant::now();

    // Use SSH options optimized for connectivity testing
    let ssh_options = crate::ssh::SshConnectionOptions::for_connectivity_test();

    while start_time.elapsed() < timeout {
        // Try to connect via SSH and run a simple command using the centralized SSH function
        let status = crate::ssh::connect_via_container_with_options(
            container_name,
            vec!["true".to_string()], // Just run 'true' to test connectivity
            &ssh_options,
        );

        if let Ok(exit_status) = status {
            if exit_status.success() {
                debug!("SSH connection successful, VM is ready");
                return Ok(());
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(color_eyre::eyre::eyre!(
        "Timeout waiting for SSH connectivity after {}s",
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

    // Wait for systemd to signal readiness or fall back to SSH polling
    let has_systemd_notify = wait_for_systemd_ready(&container_name, Duration::from_secs(60))?;

    if !has_systemd_notify {
        // Fall back to SSH polling for older systemd versions
        debug!("Falling back to SSH polling for VM readiness");
        wait_for_ssh_ready(&container_name, Duration::from_secs(60))?;
    }

    // Execute SSH connection directly (no thread needed for this)
    // This allows SSH output to be properly forwarded to stdout/stderr
    debug!("Connecting to SSH...");
    let status = ssh::connect_via_container_with_status(&container_name, opts.ssh_args)?;
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
