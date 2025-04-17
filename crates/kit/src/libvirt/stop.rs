//! libvirt stop command - stop a running bootc domain
//!
//! This module provides functionality to stop running libvirt domains
//! that were created from bootc container images.

use clap::Parser;
use color_eyre::Result;

/// Options for stopping a libvirt domain
#[derive(Debug, Parser)]
pub struct LibvirtStopOpts {
    /// Name of the domain to stop
    pub name: String,

    /// Force stop the domain
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Timeout in seconds for graceful shutdown
    #[clap(long, default_value = "60")]
    pub timeout: u32,
}

/// Execute the libvirt stop command
pub fn run(opts: LibvirtStopOpts) -> Result<()> {
    // Convert LibvirtStopOpts to podman_bootc::StopOpts and delegate
    let pb_opts = crate::podman_bootc::StopOpts {
        name: opts.name,
        force: opts.force,
        timeout: opts.timeout,
    };

    crate::podman_bootc::stop_vm(pb_opts)
}
