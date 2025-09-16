//! Ephemeral VM management commands
//!
//! This module provides subcommands for running bootc containers as ephemeral virtual machines.
//! Ephemeral VMs are temporary, non-persistent VMs that are useful for testing, development,
//! and CI/CD workflows.

use clap::Subcommand;
use color_eyre::Result;

// Re-export the existing implementations
use crate::run_ephemeral;
use crate::run_ephemeral_ssh;
use crate::ssh;

/// SSH connection options for accessing running VMs.
///
/// Provides secure shell access to VMs running within containers,
/// with automatic key management and connection routing.
#[derive(clap::Parser, Debug)]
pub struct SshOpts {
    /// Name or ID of the container running the target VM
    ///
    /// This should match the container name from podman or the VM ID
    /// used when starting the ephemeral VM.
    pub container_name: String,

    /// Additional SSH client arguments to pass through
    ///
    /// Standard ssh arguments like -v for verbose output, -L for
    /// port forwarding, or -o for SSH options.
    #[clap(allow_hyphen_values = true, help = "SSH arguments like -v, -L, -o")]
    pub args: Vec<String>,
}

/// Ephemeral VM operations
#[derive(Debug, Subcommand)]
pub enum EphemeralCommands {
    /// Run bootc containers as ephemeral VMs
    #[clap(name = "run")]
    Run(run_ephemeral::RunEphemeralOpts),

    /// Run ephemeral VM and SSH into it
    #[clap(name = "run-ssh")]
    RunSsh(run_ephemeral_ssh::RunEphemeralSshOpts),

    /// Connect to running VMs via SSH
    #[clap(name = "ssh")]
    Ssh(SshOpts),
}

impl EphemeralCommands {
    /// Execute the ephemeral subcommand
    pub fn run(self) -> Result<()> {
        match self {
            EphemeralCommands::Run(opts) => run_ephemeral::run(opts),
            EphemeralCommands::RunSsh(opts) => run_ephemeral_ssh::run_ephemeral_ssh(opts),
            EphemeralCommands::Ssh(opts) => {
                // Wait for systemd to signal readiness or fall back to SSH polling
                let has_systemd_notify = run_ephemeral_ssh::wait_for_systemd_ready(
                    &opts.container_name,
                    std::time::Duration::from_secs(60),
                )?;

                if !has_systemd_notify {
                    // Fall back to SSH polling for older systemd versions
                    run_ephemeral_ssh::wait_for_ssh_ready(
                        &opts.container_name,
                        std::time::Duration::from_secs(60),
                    )?;
                }

                ssh::connect_via_container(&opts.container_name, opts.args)
            }
        }
    }
}
