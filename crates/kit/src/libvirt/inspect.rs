//! libvirt inspect command - show detailed information about a bootc domain
//!
//! This module provides functionality to display detailed information about
//! libvirt domains that were created from bootc container images.

use clap::Parser;
use color_eyre::Result;

/// Options for inspecting a libvirt domain
#[derive(Debug, Parser)]
pub struct LibvirtInspectOpts {
    /// Name of the domain to inspect
    pub name: String,

    /// Output format
    #[clap(long, default_value = "yaml")]
    pub format: String,
}

/// Execute the libvirt inspect command
pub fn run(opts: LibvirtInspectOpts) -> Result<()> {
    // Convert LibvirtInspectOpts to podman_bootc::InspectOpts and delegate
    let pb_opts = crate::podman_bootc::InspectOpts {
        name: opts.name,
        format: opts.format,
    };

    crate::podman_bootc::inspect_vm(pb_opts)
}
