//! libvirt run command - run a bootable container as a persistent VM
//!
//! This module provides the core functionality for creating and managing
//! libvirt-based VMs from bootc container images. This is the same
//! functionality as `bcvk pb run` but organized under the libvirt subcommand.

use clap::Parser;
use color_eyre::Result;

/// Options for creating and running a bootable container VM
#[derive(Debug, Parser)]
pub struct LibvirtRunOpts {
    /// Container image to run as a bootable VM
    pub image: String,

    /// Name for the VM (auto-generated if not specified)
    #[clap(long)]
    pub name: Option<String>,

    /// Memory size in MB for the VM
    #[clap(long, default_value = "2048")]
    pub memory: u32,

    /// Number of virtual CPUs for the VM
    #[clap(long, default_value = "2")]
    pub cpus: u32,

    /// Disk size in GB for the VM
    #[clap(long, default_value = "20")]
    pub disk_size: u32,

    /// Root filesystem type for installation
    #[clap(long, default_value = "ext4")]
    pub filesystem: String,

    /// Port mapping from host to VM
    #[clap(long = "port", short = 'p', action = clap::ArgAction::Append)]
    pub port_mappings: Vec<String>,

    /// Volume mount from host to VM
    #[clap(long = "volume", short = 'v', action = clap::ArgAction::Append)]
    pub volumes: Vec<String>,

    /// Network mode for the VM
    #[clap(long, default_value = "user")]
    pub network: String,

    /// Keep the VM running in background after creation
    #[clap(long)]
    pub detach: bool,

    /// Automatically SSH into the VM after creation
    #[clap(long)]
    pub ssh: bool,
}

/// Execute the libvirt run command
pub fn run(opts: LibvirtRunOpts) -> Result<()> {
    // Convert LibvirtRunOpts to podman_bootc::RunOpts and use the implementation
    let pb_opts = crate::podman_bootc::RunOpts {
        image: opts.image,
        name: opts.name,
        memory: opts.memory,
        cpus: opts.cpus,
        disk_size: opts.disk_size,
        filesystem: opts.filesystem,
        port_mappings: opts.port_mappings,
        volumes: opts.volumes,
        network: opts.network,
        detach: opts.detach,
        ssh: opts.ssh,
    };

    // Call the actual implementation
    crate::podman_bootc::run_vm_impl(pb_opts)
}
