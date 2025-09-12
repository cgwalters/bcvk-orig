use color_eyre::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
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

/// Run an ephemeral pod and immediately SSH into it, with lifecycle binding
pub fn run_ephemeral_ssh(opts: RunEphemeralSshOpts) -> Result<()> {
    // Start the ephemeral pod in detached mode with SSH enabled
    let mut ephemeral_opts = opts.run_opts.clone();
    ephemeral_opts.podman.detach = true;
    ephemeral_opts.common.ssh_keygen = true; // Enable SSH key generation and access

    debug!("Starting ephemeral VM...");
    let container_id = run_detached(ephemeral_opts)?;
    debug!("Ephemeral VM started with container ID: {}", container_id);

    // Use the container ID for SSH and cleanup
    let container_name = container_id;
    debug!("Using container ID: {}", container_name);

    // No need for threading or coordination flags since we'll cleanup synchronously

    // Give the VM time to boot (VMs take longer than containers)
    debug!("Waiting for VM to boot...");
    thread::sleep(Duration::from_secs(20));

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
        .args(["stop", &container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new("podman")
        .args(["rm", "-f", &container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Exit with SSH client's exit code
    std::process::exit(exit_code);
}
