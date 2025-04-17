//! libvirt start command - start a stopped bootc domain
//!
//! This module provides functionality to start stopped libvirt domains
//! that were created from bootc container images.

use clap::Parser;
use color_eyre::Result;

/// Options for starting a libvirt domain
#[derive(Debug, Parser)]
pub struct LibvirtStartOpts {
    /// Name of the domain to start
    pub name: String,

    /// Automatically SSH into the domain after starting
    #[clap(long)]
    pub ssh: bool,
}

/// Execute the libvirt start command
pub fn run(opts: LibvirtStartOpts) -> Result<()> {
    // Convert LibvirtStartOpts to podman_bootc::StartOpts and delegate
    let pb_opts = crate::podman_bootc::StartOpts {
        name: opts.name,
        ssh: opts.ssh,
    };

    crate::podman_bootc::start_vm(pb_opts)
}
