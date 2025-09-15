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
pub mod list;
pub mod run;
pub mod ssh;
pub mod upload;

/// libvirt subcommands for managing bootc disk images and domains
#[derive(Debug, Subcommand)]
pub enum LibvirtCommands {
    /// Run a bootable container as a persistent VM
    ///
    /// Creates a complete libvirt-managed VM from a bootc container image.
    /// Automatically handles disk image creation, domain setup, SSH key injection,
    /// and VM lifecycle management. This is equivalent to 'bcvk pb run'.
    Run(run::LibvirtRunOpts),

    /// Upload bootc disk images to libvirt with metadata annotations
    ///
    /// Combines run-install with libvirt integration to create and upload
    /// disk images to libvirt storage pools. Automatically adds container
    /// image metadata as libvirt annotations for tracking and management.
    Upload(upload::LibvirtUploadOpts),

    /// Create and start domains from uploaded bootc volumes
    ///
    /// Creates libvirt domains using existing bootc volumes in storage pools.
    /// Automatically configures domains with appropriate resources, networking,
    /// and console access. Can optionally start the domain after creation.
    Create(create::LibvirtCreateOpts),

    /// List available bootc volumes with metadata
    ///
    /// Discovers bootc volumes in libvirt storage pools and displays their
    /// container image metadata and creation information. Supports both
    /// human-readable and JSON output formats.
    List(list::LibvirtListOpts),

    /// SSH to libvirt domain with embedded SSH key
    ///
    /// Connects to libvirt domains that were created with SSH key injection.
    /// Automatically retrieves SSH credentials from domain XML metadata and
    /// establishes connection using embedded private key.
    Ssh(ssh::LibvirtSshOpts),
}

impl LibvirtCommands {
    pub fn run(self) -> Result<()> {
        match self {
            LibvirtCommands::Run(opts) => run::run(opts),
            LibvirtCommands::Upload(opts) => upload::run(opts),
            LibvirtCommands::Create(opts) => create::run(opts),
            LibvirtCommands::List(opts) => list::run(opts),
            LibvirtCommands::Ssh(opts) => ssh::run(opts),
        }
    }
}
