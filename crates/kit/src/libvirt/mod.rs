//! libvirt integration for bcvk
//!
//! This module provides a comprehensive libvirt integration with subcommands for:
//! - `upload`: Upload bootc disk images to libvirt with metadata annotations
//! - `create`: Create and start domains from uploaded volumes
//! - `list`: List available bootc volumes with metadata

use clap::Subcommand;
use color_eyre::Result;

pub mod create;
pub mod domain;
pub mod inspect;
pub mod list;
pub mod rm;
pub mod run;
pub mod ssh;
pub mod start;
pub mod stop;
pub mod upload;

/// libvirt subcommands for managing bootc disk images and domains
#[derive(Debug, Subcommand)]
pub enum LibvirtCommands {
    /// Run a bootable container as a persistent VM
    Run(run::LibvirtRunOpts),

    /// SSH to libvirt domain with embedded SSH key
    Ssh(ssh::LibvirtSshOpts),

    /// List available bootc domains with metadata
    List(list::LibvirtListOpts),

    /// Stop a running libvirt domain
    Stop(stop::LibvirtStopOpts),

    /// Start a stopped libvirt domain
    Start(start::LibvirtStartOpts),

    /// Remove a libvirt domain and its resources
    #[clap(name = "rm")]
    Remove(rm::LibvirtRmOpts),

    /// Show detailed information about a libvirt domain
    Inspect(inspect::LibvirtInspectOpts),

    /// Upload bootc disk images to libvirt with metadata annotations
    Upload(upload::LibvirtUploadOpts),

    /// Create and start domains from uploaded bootc volumes
    Create(create::LibvirtCreateOpts),
}

impl LibvirtCommands {
    pub fn run(self) -> Result<()> {
        match self {
            LibvirtCommands::Run(opts) => run::run(opts),
            LibvirtCommands::Ssh(opts) => ssh::run(opts),
            LibvirtCommands::List(opts) => list::run(opts),
            LibvirtCommands::Stop(opts) => stop::run(opts),
            LibvirtCommands::Start(opts) => start::run(opts),
            LibvirtCommands::Remove(opts) => rm::run(opts),
            LibvirtCommands::Inspect(opts) => inspect::run(opts),
            LibvirtCommands::Upload(opts) => upload::run(opts),
            LibvirtCommands::Create(opts) => create::run(opts),
        }
    }
}
