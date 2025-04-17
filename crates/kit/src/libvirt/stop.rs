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
    stop_vm_impl(opts)
}

/// Stop a running VM (implementation)
pub fn stop_vm_impl(opts: LibvirtStopOpts) -> Result<()> {
    use crate::domain_list::DomainLister;
    use color_eyre::eyre::Context;
    use std::process::Command;

    let lister = DomainLister::new();

    // Check if domain exists and get its state
    let state = lister
        .get_domain_state(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if state != "running" {
        println!("VM '{}' is already stopped (state: {})", opts.name, state);
        return Ok(());
    }

    println!("🛑 Stopping VM '{}'...", opts.name);

    // Use virsh to stop the domain
    let mut cmd = Command::new("virsh");
    if opts.force {
        cmd.args(&["destroy", &opts.name]);
    } else {
        cmd.args(&["shutdown", &opts.name]);
    }

    let output = cmd
        .output()
        .with_context(|| "Failed to run virsh command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to stop VM '{}': {}",
            opts.name,
            stderr
        ));
    }

    println!("VM '{}' stopped successfully", opts.name);
    Ok(())
}
