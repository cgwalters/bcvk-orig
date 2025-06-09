//! Module for libvirt-related functionality
//!
//! In the future we may replace this with https://gitlab.com/libvirt/libvirt-rust

use std::{process::Command, str::FromStr, sync::OnceLock};

use bootc_utils::CommandRunExt as _;
use color_eyre::{
    eyre::{self, eyre},
    Result,
};

use tracing::instrument;
use virt::{
    connect::Connect,
    domain::Domain,
    sys::{VIR_DOMAIN_PAUSED, VIR_DOMAIN_RUNNING, VIR_DOMAIN_SHUTOFF},
};

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

impl FromStr for LibvirtConnection {
    type Err = eyre::Report;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "session" | "qemu:///session" => Ok(LibvirtConnection::Session),
            "system" | "qemu:///system" => Ok(LibvirtConnection::System),
            _ => Err(eyre!("Invalid libvirt connection: {}", s)),
        }
    }
}

pub(crate) fn libvirt_storage_pool() -> &'static str {
    static POOL: OnceLock<String> = OnceLock::new();
    POOL.get_or_init(|| {
        std::env::var("LIBVIRT_STORAGE_POOL").unwrap_or_else(|_| "default".to_string())
    })
}

#[derive(clap::Parser, Debug)]
pub(crate) struct LibvirtOpts {
    /// Connection to libvirt
    #[clap(long, short = 'c', default_value = "session")]
    connection: String,

    #[clap(subcommand)]
    subcommand: LibvirtCommand,
}

impl LibvirtOpts {
    pub fn run(self) -> Result<()> {
        let conn = Connect::open(Some(&self.connection))?;
        let uri = conn.get_uri()?;
        let connection = LibvirtConnection::from_str(uri.as_str())?;
        match self.subcommand {
            LibvirtCommand::SyncCloudImage { os, force } => {
                crate::virtinstall::sync(connection, &os, force)
            }
            LibvirtCommand::InstallFromSRB(opts) => opts.run(connection),
            LibvirtCommand::List => crate::virtinstall::list_vms(&conn),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum LibvirtCommand {
    SyncCloudImage {
        os: crate::virtinstall::OperatingSystem,
        #[clap(long)]
        force: bool,
    },
    /// Create a new running libvirt virtual machine from a bootc container image.
    /// The virtual machine will be installed using `system-reinstall-bootc`
    /// starting from a default cloud image.
    InstallFromSRB(crate::virtinstall::FromSRBOpts),
    /// List virtual machines created by bootc-kit
    List,
}

pub fn virsh_command(connection: LibvirtConnection) -> Result<Command> {
    let mut r = crate::hostexec::command("virsh", None)?;
    r.args(["-c", connection.to_url()]);
    Ok(r)
}

#[instrument]
/// List all virtual machines using the virt crate
pub fn domain_list(conn: &Connect) -> Result<Vec<Domain>> {
    let r = conn.list_all_domains(
        virt::sys::VIR_CONNECT_LIST_DOMAINS_ACTIVE | virt::sys::VIR_CONNECT_LIST_DOMAINS_INACTIVE,
    )?;
    Ok(r)
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

pub(crate) fn get_state_str(state: virt::sys::virDomainState) -> Result<&'static str> {
    let state = match state {
        VIR_DOMAIN_RUNNING => "running",
        VIR_DOMAIN_PAUSED => "paused",
        VIR_DOMAIN_SHUTOFF => "shut off",
        // TODO: support other states on demand
        _ => "-",
    };
    Ok(state)
}
