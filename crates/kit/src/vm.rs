//! Module for VM-related functionality
//!
//! This contains common code used by both run-rmvm and virt-install commands

use std::process::Command;

use bootc_utils::CommandRunExt;
use color_eyre::{eyre::eyre, Result};
use tracing::instrument;

use crate::hostexec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum LibvirtConnection {
    Session,
    System,
}

impl LibvirtConnection {
    pub fn to_url(&self) -> &'static str {
        match self {
            LibvirtConnection::Session => "qemu:///session",
            LibvirtConnection::System => "qemu:///system",
        }
    }
}

impl Default for LibvirtConnection {
    fn default() -> Self {
        Self::Session
    }
}

/// Check if libvirt is available on the host
#[instrument]
pub fn check_libvirt_available() -> Result<bool> {
    let status = hostexec::command("systemctl", None)?
        .args(["is-active", "libvirtd"])
        .output()
        .map_err(|e| eyre!("Checking if libvirtd is running: {}", e))?;

    Ok(status.status.success())
}

/// Check if a VM exists by name
#[instrument]
pub fn vm_exists(connection: LibvirtConnection, name: &str) -> Result<bool> {
    let output = virsh_command(connection)?
        .args(["list", "--all", "--name"])
        .run_get_string()
        .map_err(|e| eyre!("Listing VMs: {}", e))?;

    Ok(output.lines().any(|line| line.trim() == name))
}

pub fn virsh_command(connection: LibvirtConnection) -> Result<Command> {
    let mut r = hostexec::command("virsh", None)?;
    r.args(["-c", connection.to_url()]);
    Ok(r)
}

/// Delete a VM
#[instrument]
pub fn delete_vm(connection: LibvirtConnection, name: &str) -> Result<()> {
    println!("Deleting VM {}...", name);
    virsh_command(connection)?
        .args(["destroy", name])
        .run()
        .map_err(|e| eyre!("Destroying VM {}: {}", name, e))?;
    virsh_command(connection)?
        .args(["undefine", name, "--remove-all-storage", "--tpm"])
        .run()
        .map_err(|e| eyre!("Deleting VM {}: {}", name, e))?;

    Ok(())
}

/// Get IP address of a VM
#[instrument]
pub fn get_vm_ip(name: &str) -> Result<Option<String>> {
    let output = hostexec::command("virsh", None)?
        .args(["domifaddr", name])
        .output()
        .map_err(|e| eyre!("Getting VM IP address: {}", e))?;

    if !output.status.success() {
        return Err(eyre!("Failed to get IP address for VM {}", name));
    }

    // Parse the output to find the IP address
    // Example output:
    // Name       MAC address          Protocol     Address
    // -------------------------------------------------------
    // vnet0      52:54:00:01:02:03    ipv4         192.168.122.2/24

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines().skip(2) {
        // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let addr = parts[3];
            if let Some((ip, _)) = addr.split_once('/') {
                return Ok(Some(ip.to_string()));
            }
        }
    }

    Ok(None) // No IP address found
}
