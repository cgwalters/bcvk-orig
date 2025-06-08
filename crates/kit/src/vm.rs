//! Module for libvirt-related functionality
//!
//! In the future we may replace this with https://gitlab.com/libvirt/libvirt-rust

use std::{fmt::Display, process::Command, str::FromStr};

use bootc_utils::CommandRunExt;
use color_eyre::{
    eyre::{self, eyre},
    Result,
};
use itertools::Itertools;
use tracing::instrument;

use crate::hostexec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum LibvirtConnection {
    #[clap(alias = "qemu:///session")]
    Session,
    #[clap(alias = "qemu:///system")]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VMState {
    Running,
    Paused,
    Stopped,
    Other(String),
}

impl FromStr for VMState {
    type Err = eyre::Report;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(VMState::Running),
            "paused" => Ok(VMState::Paused),
            "shut off" => Ok(VMState::Stopped),
            other => Ok(VMState::Other(other.to_string())),
        }
    }
}

impl Display for VMState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMState::Running => write!(f, "running"),
            VMState::Paused => write!(f, "paused"),
            VMState::Stopped => write!(f, "shut off"),
            VMState::Other(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibvirtVM {
    pub id: Option<u64>,
    pub name: String,
    pub state: VMState,
}

impl LibvirtVM {
    pub fn is_running(&self) -> bool {
        matches!(self.state, VMState::Running)
    }
}

pub fn virsh_command(connection: LibvirtConnection) -> Result<Command> {
    let mut r = hostexec::command("virsh", None)?;
    r.args(["-c", connection.to_url()]);
    Ok(r)
}

fn parse_domain_list(output: &str) -> Result<Vec<LibvirtVM>> {
    output
        .lines()
        .skip(2)
        .try_fold(Vec::new(), |mut acc, line| {
            // Skip empty lines
            if line.trim().is_empty() {
                return Ok(acc);
            }

            let mut parts = line.split_whitespace();
            let Some(id) = parts.next() else {
                return Err(eyre!("Invalid output from virsh list: {line}"));
            };
            let id = if id == "-" {
                None
            } else {
                Some(id.parse::<u64>()?)
            };
            let Some(name) = parts.next().map(ToOwned::to_owned) else {
                return Err(eyre!("Invalid output from virsh list: {line}"));
            };
            let state: String = parts.join(" ");
            let state = VMState::from_str(&state)?;
            acc.push(LibvirtVM { id, name, state });
            Ok(acc)
        })
}

#[instrument]
/// List all virtual machines
pub fn domain_list(connection: LibvirtConnection) -> Result<Vec<LibvirtVM>> {
    let output = virsh_command(connection)?
        .args(["list", "--all"])
        .run_get_string()
        .map_err(|e| eyre!("Listing VMs: {}", e))?;
    parse_domain_list(&output)
}

#[instrument]
pub fn domain_exists(connection: LibvirtConnection, id: &str) -> Result<bool> {
    let r = virsh_command(connection)?
        .args(["dominfo", id])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    Ok(r?.success())
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DomifAddr {
    pub name: String,
    pub mac: String,
    pub proto: String,
    pub addr: String,
}

/// Get IP address of a VM
#[instrument]
pub fn get_vm_domifaddr(connection: LibvirtConnection, name: &str) -> Result<Option<DomifAddr>> {
    let output = virsh_command(connection)?
        .args(["domifaddr", name])
        .run_get_string()
        .map_err(|e| eyre!("Getting VM IP address: {}", e))?;
    let mut output = output.lines().skip(2);
    let Some(domifaddr) = output.next() else {
        return Ok(None);
    };
    let [name, mac, proto, addr] = domifaddr
        .split_ascii_whitespace()
        .collect_array()
        .ok_or_else(|| eyre!("Failed to parse domifaddr: {domifaddr}"))?;
    let r = DomifAddr {
        name: name.to_string(),
        mac: mac.to_string(),
        proto: proto.to_string(),
        addr: addr.to_string(),
    };
    Ok(Some(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domain_list_empty() {
        let input = " Id   Name                      State
------------------------------------------
";
        let result = parse_domain_list(input).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_domain_list_single() {
        let input = " Id   Name                      State
------------------------------------------
 1    test-vm                   running";
        let result = parse_domain_list(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-vm");
        assert_eq!(result[0].state, VMState::Running);
    }

    #[test]
    fn test_parse_domain_list_multiple() {
        let input = " Id   Name                      State
------------------------------------------
 1    test-vm1                  running
 2    test-vm2                  paused
 -    test-vm3                  shut off";
        let result = parse_domain_list(input).unwrap();
        assert_eq!(result.len(), 3);

        assert_eq!(result[0].name, "test-vm1");
        assert_eq!(result[0].state, VMState::Running);

        assert_eq!(result[1].name, "test-vm2");
        assert_eq!(result[1].state, VMState::Paused);

        assert_eq!(result[2].name, "test-vm3");
        assert_eq!(result[2].state, VMState::Stopped);
    }

    #[test]
    fn test_parse_domain_list_invalid_state() {
        let input = " Id   Name                      State
------------------------------------------
 1    test-vm                   unknown";
        let result = parse_domain_list(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-vm");
        if let VMState::Other(state) = &result[0].state {
            assert_eq!(state, "unknown");
        } else {
            panic!("Expected VMState::Other");
        }
    }
}
