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
    inspect_vm_impl(opts)
}

/// Show detailed information about a VM (implementation)
pub fn inspect_vm_impl(opts: LibvirtInspectOpts) -> Result<()> {
    use crate::domain_list::DomainLister;
    use color_eyre::eyre::Context;

    let lister = DomainLister::new();

    // Get domain info
    let vm = lister
        .get_domain_info(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    match opts.format.as_str() {
        "yaml" => {
            println!("name: {}", vm.name);
            if let Some(ref image) = vm.image {
                println!("image: {}", image);
            }
            println!("status: {}", vm.status_string());
            if let Some(memory) = vm.memory_mb {
                println!("memory_mb: {}", memory);
            }
            if let Some(vcpus) = vm.vcpus {
                println!("vcpus: {}", vcpus);
            }
            if let Some(ref disk_path) = vm.disk_path {
                println!("disk_path: {}", disk_path);
            }
        }
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&vm)
                    .with_context(|| "Failed to serialize VM as JSON")?
            );
        }
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "Unsupported format: {}",
                opts.format
            ))
        }
    }
    Ok(())
}
