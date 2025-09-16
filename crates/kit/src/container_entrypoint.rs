use clap::{Parser, Subcommand};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tokio::signal::unix::SignalKind;
use tracing::debug;

use crate::run_ephemeral::RunEphemeralOpts;

#[derive(Parser)]
pub struct ContainerEntrypointOpts {
    #[command(subcommand)]
    pub command: ContainerCommands,
}

#[derive(Subcommand)]
pub enum ContainerCommands {
    /// Run ephemeral VM (what run-ephemeral-impl does today)
    RunEphemeral,

    /// SSH to VM from container
    Ssh(SshOpts),
}

#[derive(Parser)]
pub struct SshOpts {
    /// SSH arguments  
    #[clap(allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Configuration passed via BCK_CONFIG environment variable
#[derive(Serialize, Deserialize)]
pub struct ContainerConfig {
    pub memory_mb: u32,
    pub vcpus: u32,
    pub console: bool,
    pub extra_args: Option<String>,
    // Future: SSH config, etc.
}

pub async fn run_ephemeral_in_container() -> Result<()> {
    // Parse BCK_CONFIG from environment
    let config_json = std::env::var("BCK_CONFIG")?;
    let opts: RunEphemeralOpts = serde_json::from_str(&config_json)?;

    // Call existing run_impl
    crate::run_ephemeral::run_impl(opts).await
}

pub fn ssh_to_vm(opts: SshOpts) -> Result<()> {
    debug!("SSH to VM with args: {:?}", opts.args);

    // SSH implementation
    // Default to root@10.0.2.15 (QEMU user networking)
    let mut cmd = std::process::Command::new("ssh");

    // Check if SSH key exists
    if std::path::Path::new("/tmp/ssh").exists() {
        cmd.args(["-i", "/tmp/ssh"]);
    }

    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    cmd.args(["-o", "UserKnownHostsFile=/dev/null"]);
    cmd.args(["-o", "LogLevel=ERROR"]); // Reduce SSH verbosity

    // If no host specified in args, use default
    if !opts.args.iter().any(|arg| arg.contains("@")) {
        cmd.arg("root@10.0.2.15");
    }

    // Add any additional arguments
    if !opts.args.is_empty() && !opts.args.iter().any(|arg| arg.contains("@")) {
        cmd.arg("--");
    }
    cmd.args(&opts.args);

    let status = cmd.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

pub async fn run(opts: ContainerEntrypointOpts) -> Result<()> {
    let mut term = tokio::signal::unix::signal(SignalKind::from_raw(libc::SIGRTMIN() + 3))?;

    tokio::select! {
        _ = term.recv() => {
            debug!("Caught SIGRTMIN+3");
            std::process::exit(0)
        }
        r = async {
            match opts.command {
                ContainerCommands::RunEphemeral => run_ephemeral_in_container().await,
                ContainerCommands::Ssh(ssh_opts) => {
                    tokio::task::spawn_blocking(move || ssh_to_vm(ssh_opts)).await?
                }
            }
        } => r
    }
}
