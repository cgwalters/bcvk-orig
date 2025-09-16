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
    start_vm_impl(opts)
}

/// Start a stopped VM (implementation)
pub fn start_vm_impl(opts: LibvirtStartOpts) -> Result<()> {
    use crate::domain_list::DomainLister;
    use color_eyre::eyre::Context;
    use std::process::Command;

    let lister = DomainLister::new();

    // Check if domain exists and get its state
    let state = lister
        .get_domain_state(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if state == "running" {
        println!("VM '{}' is already running", opts.name);
        if opts.ssh {
            println!("🔗 Connecting to running VM...");
            let ssh_opts = crate::libvirt::ssh::LibvirtSshOpts {
                domain_name: opts.name,
                connect: None,
                user: "root".to_string(),
                command: vec![],
                strict_host_keys: false,
                timeout: 30,
            };
            return crate::libvirt::ssh::run(ssh_opts);
        }
        return Ok(());
    }

    println!("Starting VM '{}'...", opts.name);

    // Use virsh to start the domain
    let output = Command::new("virsh")
        .args(&["start", &opts.name])
        .output()
        .with_context(|| "Failed to run virsh start")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to start VM '{}': {}",
            opts.name,
            stderr
        ));
    }

    println!("VM '{}' started successfully", opts.name);

    if opts.ssh {
        println!("🔗 Use 'bcvk libvirt ssh {}' to connect", opts.name);
    }

    Ok(())
}
