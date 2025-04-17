//! libvirt rm command - remove a bootc domain and its resources
//!
//! This module provides functionality to permanently remove libvirt domains
//! and their associated disk images that were created from bootc container images.

use clap::Parser;
use color_eyre::Result;

/// Options for removing a libvirt domain
#[derive(Debug, Parser)]
pub struct LibvirtRmOpts {
    /// Name of the domain to remove
    pub name: String,

    /// Force removal without confirmation
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Remove domain even if it's running
    #[clap(long)]
    pub stop: bool,
}

/// Execute the libvirt rm command
pub fn run(opts: LibvirtRmOpts) -> Result<()> {
    // Convert LibvirtRmOpts to podman_bootc::RemoveOpts and delegate
    let pb_opts = crate::podman_bootc::RemoveOpts {
        name: opts.name,
        force: opts.force,
        stop: opts.stop,
    };

    crate::podman_bootc::remove_vm(pb_opts)
}
