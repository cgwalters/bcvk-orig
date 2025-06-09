use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};

use bootc_utils::CommandRunExt;
use clap::Parser;
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use indicatif::{ProgressBar, ProgressStyle};

use tracing::instrument;
use virt::connect::Connect;

use crate::init::DEFAULT_CSTOR_DIST_PORT;
use crate::libvirt::{libvirt_storage_pool, virsh_command, LibvirtConnection};
use crate::{hostexec, images, sshcred};

const REINSTALL_SCRIPT: &str = include_str!("reinstall.py");

#[derive(Debug, Clone, Default, clap::Args)]
pub(crate) struct LibvirtGenericOpts {}

#[derive(Debug, Clone, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub(crate) enum OperatingSystem {
    Fedora,
    CentOSStream10,
}

impl OperatingSystem {
    const fn cloud_url(&self) -> &'static str {
        match self {
            Self::Fedora => const_format::concatcp!(
                "https://download.fedoraproject.org/pub/fedora/linux/releases/42/Cloud/",
                std::env::consts::ARCH,
                "/images/Fedora-Cloud-Base-Generic-42-1.1.",
                std::env::consts::ARCH,
                ".qcow2"
            ),
            Self::CentOSStream10 => const_format::concatcp!(
                "https://cloud.centos.org/centos/10-stream/",
                std::env::consts::ARCH,
                "/images/CentOS-Stream-GenericCloud-",
                std::env::consts::ARCH,
                "-10-20250529.0.",
                std::env::consts::ARCH,
                ".qcow2"
            ),
        }
    }

    fn libvirt_name(&self) -> &'static str {
        match self {
            Self::Fedora => "fedora-42-cloud.qcow2",
            Self::CentOSStream10 => "centos-stream-10-cloud.qcow2",
        }
    }

    fn osinfo_name(&self) -> &'static str {
        match self {
            OperatingSystem::Fedora => "fedora41",
            OperatingSystem::CentOSStream10 => "centos-stream10",
        }
    }

    fn from_osrelease(osrelease: &HashMap<String, String>) -> Option<Self> {
        let Some(id) = osrelease.get("ID") else {
            return None;
        };
        if id == "fedora" {
            return Some(Self::Fedora);
        }
        let id_like = osrelease
            .get("ID_LIKE")
            .into_iter()
            .flat_map(|v| v.split_ascii_whitespace())
            .collect::<Vec<&str>>();
        if id_like.contains(&"rhel") {
            return Some(Self::CentOSStream10);
        } else if id_like.contains(&"fedora") {
            return Some(Self::Fedora);
        } else {
            None
        }
    }
}

#[derive(Parser, Debug)]
pub struct FromSRBOpts {
    /// Name of the image to install
    pub image: String,

    /// Name for the virtual machine
    pub name: String,

    /// Set to true to fetch directly from a remote regist/*  */ry
    #[clap(long)]
    pub remote: bool,

    /// This virtual machine should not persist across host reboots
    #[clap(long)]
    pub transient: bool,

    /// Do not bind the container storage via virtiofs
    #[clap(long)]
    pub skip_bind_storage: bool,

    /// Instead of using a default cloud image associated
    /// with the container image OS, use this libvirt volume
    /// which should hold an image.
    #[clap(long)]
    pub base_volume: Option<String>,

    /// Automatically destroy an existing VM with this name
    #[clap(long, short = 'D')]
    pub autodestroy: bool,

    /// Path to SSH key
    #[clap(long)]
    pub sshkey: Option<String>,

    /// Size of the root volume in GiB
    #[clap(long, default_value_t = 10)]
    pub size: u32,

    #[clap(long, default_value_t = 2)]
    pub vcpus: u32,

    #[clap(long, default_value = "4096")]
    pub memory: u32,

    /// Pass through this argument to virt-install
    #[clap(long, short = 'a')]
    pub vinstarg: Vec<String>,
}

#[instrument]
pub(crate) fn list_vms(conn: &Connect, connection: LibvirtConnection) -> Result<()> {
    let domains = crate::libvirt::domain_list(connection)?;
    for domain in domains {
        let name = domain.name.as_str();
        let state = &domain.state;
        let desc = virsh_command(connection)?
            .args(["desc", name])
            .run_get_string()
            .map_err(|e| eyre!("Failed to get description of VM: {e}"))?;
        if desc.contains("bootc-kit") {
            if domain.is_running() {
                let domifaddr = crate::libvirt::get_vm_domifaddr(connection, name)?;
                let ip = domifaddr
                    .as_ref()
                    .and_then(|v| v.addr.rsplit_once('/'))
                    .map(|v| v.0)
                    .unwrap_or("-");
                println!("{name} {state} {ip}");
            } else {
                println!("{name} {state}");
            }
        }
    }
    Ok(())
}

#[instrument]
pub(crate) fn sync(connection: LibvirtConnection, os: &OperatingSystem, force: bool) -> Result<()> {
    let vol = os.libvirt_name();
    let exists = virsh_command(connection)?
        .args(["vol-info", "--pool", libvirt_storage_pool(), vol])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success();
    if exists {
        if !force {
            println!("Volume already present: {vol}");
            return Ok(());
        } else {
            virsh_command(connection)?
                .args(["vol-delete", "--pool", libvirt_storage_pool(), vol])
                .run()
                .map_err(|e| eyre!("Failed to delete volume: {e}"))?;
        }
    }

    let url = os.cloud_url();
    tracing::debug!("Fetching {url}");
    let client = reqwest::blocking::ClientBuilder::new()
        .user_agent(format!("bootc-kit/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap();
    let r = client
        .get(url)
        .send()
        .and_then(|v| v.error_for_status())
        .wrap_err_with(|| format!("Fetching {url}"))?;
    let Some(size) = r.content_length() else {
        return Err(eyre!("No content length"));
    };
    tracing::debug!("size={size}");
    let size_str = format!("{size}");
    virsh_command(connection)?
        .args([
            "vol-create-as",
            "--format",
            "qcow2",
            libvirt_storage_pool(),
            vol,
            size_str.as_str(),
        ])
        .run()
        .map_err(|e| eyre!("Failed to create volume: {e}"))?;
    let tempdir = tempfile::tempdir()?;
    let tempdir = tempdir.path().to_str().unwrap();
    // Indirect through a named pipe because libvirt uploads want a file,
    // but we don't want to download the whole thing and then upload to libvirt
    let fifopath = &format!("{tempdir}/libvirt-upload.fifo");
    Command::new("mkfifo")
        .arg(fifopath)
        .run()
        .map_err(|e| eyre!("Creating fifo: {e}"))?;
    let mut uploader = virsh_command(connection)?
        .args(["vol-upload", vol, fifopath.as_str(), libvirt_storage_pool()])
        .stdout(Stdio::null())
        .spawn()?;
    let mut fifo = OpenOptions::new()
        .write(true)
        .open(&fifopath)
        .wrap_err("Opening fifo")?;
    let pb = ProgressBar::new(size);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes})",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    let mut r = pb.wrap_read(r);
    std::io::copy(&mut r, &mut fifo).wrap_err("Fetching and uploading to libvirt")?;
    drop(fifo);
    pb.finish_and_clear();
    let st = uploader.wait()?;
    if !st.success() {
        return Err(eyre!("Failed to upload to libvirt: {st:?}"));
    }
    Ok(())
}

fn vol_path(connection: LibvirtConnection, name: &str) -> Result<String> {
    let r = virsh_command(connection)?
        .args(["vol-path", name, libvirt_storage_pool()])
        .run_get_string()
        .map_err(|e| eyre!("Failed to query volume path: {e}"))?;
    Ok(r.trim().to_owned())
}

#[allow(dead_code)]
fn vol_qcow2_clone(connection: LibvirtConnection, name: &str, newname: &str) -> Result<()> {
    let srcpath = vol_path(connection, name)?;
    let target_dir = Path::new(&srcpath).parent().unwrap();
    let target_path = target_dir.join(newname);
    let target_path = target_path.to_str().unwrap();
    let mut r = hostexec::command("qemu-img", None)?;
    r.args([
        "create",
        "-f",
        "qcow2",
        "-b",
        srcpath.as_str(),
        "-F",
        "qcow2",
        target_path,
    ]);
    r.run().map_err(|e| eyre!("Failed to clone volume: {e}"))?;
    hostexec::command("chcon", None)?
        .args(["--reference", srcpath.as_str(), target_path])
        .run()
        .map_err(|e| eyre!("Failed to chcon: {e}"))?;
    Ok(())
}

/// Given the container image, generate a cloud-init config with boot commands
/// which injects our shell script to provision.
fn template_cloudinit(image: &str, local: bool) -> Result<String> {
    use yaml_rust2::{yaml, Yaml};

    let port = if local {
        Cow::Owned(format!("{DEFAULT_CSTOR_DIST_PORT}"))
    } else {
        Cow::Borrowed("")
    };

    // Make the cloud-init config as yaml
    let mut v = yaml_rust2::yaml::Hash::new();

    // Generate write_files
    {
        let mut writefiles_entry = yaml::Hash::new();
        writefiles_entry.insert(
            Yaml::String("path".into()),
            Yaml::String("/usr/local/bin/bootc-cloudinit-entrypoint".into()),
        );
        writefiles_entry.insert(
            Yaml::String("permissions".into()),
            Yaml::String("0755".into()),
        );
        writefiles_entry.insert(
            Yaml::String("content".into()),
            Yaml::String(REINSTALL_SCRIPT.into()),
        );
        let mut writefiles = yaml::Array::new();
        writefiles.push(Yaml::Hash(writefiles_entry));
        v.insert(Yaml::String("write_files".into()), Yaml::Array(writefiles));
    }
    // Generate runcmd
    {
        // bootcmd is an array of strings
        let mut cmds = yaml::Array::new();
        cmds.push(Yaml::String(
            format!("env BOOTC_TARGET_IMAGE={image} BOOTC_CSTOR_DIST_PORT={port} /usr/local/bin/bootc-cloudinit-entrypoint")
        ));

        v.insert(Yaml::String("runcmd".into()), Yaml::Array(cmds));
    }

    // Serialize it to a string
    let mut out_str = String::new();
    let mut emitter = yaml_rust2::YamlEmitter::new(&mut out_str);
    emitter.dump(&yaml::Yaml::Hash(v))?;

    // Prefix with the magic comment
    out_str.insert_str(0, "#cloud-config\n");
    Ok(out_str)
}

impl FromSRBOpts {
    pub fn run(self, connection: LibvirtConnection) -> Result<()> {
        let vmname = self.name.as_str();
        let image = self.image.as_str();

        if self.autodestroy {
            if crate::libvirt::domain_exists(connection, vmname)? {
                println!("Destroying existing VM: {}", vmname);
                crate::libvirt::delete_vm(connection, vmname)?;
            } else {
                println!("No existing VM to autodestroy: {vmname}");
            }
        }

        // For session installs, it's a pain to deal with the TCP port allocation
        // across reboots, so just make the domain always transient.
        let transient = self.transient || connection == LibvirtConnection::Session;

        println!("Installing via system-reinstall-bootc: {image}");

        let _inspect = images::inspect(image)?;
        let osrelease = images::query_osrelease(image)?;
        let os = OperatingSystem::from_osrelease(&osrelease)
            .ok_or_else(|| eyre!("Failed to determine compatible cloud image from {image}"))?;

        let volname = if let Some(base) = self.base_volume.as_deref() {
            base
        } else {
            // Ensure we have a cloud image corresponding to this OS
            sync(connection, &os, false)?;
            os.libvirt_name()
        };
        let volpath = vol_path(connection, volname)?;

        let mut qemu_commandline = Vec::new();
        let mut vinstall = hostexec::command("virt-install", None)?;
        vinstall.args(["--connect", connection.to_url()]);
        vinstall.args([
            "--import",
            "--noautoconsole",
            "--memorybacking=source.type=memfd,access.mode=shared",
        ]);
        vinstall.args(transient.then_some("--transient"));
        vinstall.arg(format!("--os-variant={}", os.osinfo_name()));
        vinstall.arg(format!("--name={vmname}"));
        vinstall.arg(format!(
            "--metadata=description=bootc-kit: cloud installation of {image}"
        ));
        vinstall.arg(format!("--memory={}", self.memory));
        vinstall.arg(format!("--vcpus={}", self.vcpus));
        vinstall.arg(format!("--disk=size={},backing_store={volpath}", self.size));

        // Handle usermode port forwarding
        let port = if connection == LibvirtConnection::Session {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let port = listener.local_addr()?.port();
            qemu_commandline.push(format!("-netdev user,id=u0,hostfwd=tcp::{port}-:22"));
            Some(listener)
        } else {
            None
        };
        let key_contents = if let Some(path) = self.sshkey.as_deref() {
            // Need to read this from the host context
            let mut r = hostexec::command("cat", None)?
                .arg(path)
                .run_get_string()
                .map_err(|e| eyre!("Failed to read SSH key: {e}"))?;
            while r.ends_with('\n') {
                r.pop();
            }
            Some(r)
        } else {
            None
        };
        if let Some(key) = key_contents.as_ref() {
            let cred = sshcred::credential_for_root_ssh(key)?;
            qemu_commandline.push(format!("-smbios type=11,value={cred}"));
        }
        let qemu_commandline = qemu_commandline.join(" ");
        if !qemu_commandline.is_empty() {
            // Note that the way this is implemented through virt-install won't handle spaces in arguments,
            // but we really shouldn't have any of those.
            vinstall.arg(format!("--qemu-commandline={qemu_commandline}"));
        }

        let cloudinit = template_cloudinit(image, !self.remote)?;
        let mut cloud_init_tmpf = tempfile::NamedTempFile::new()?;
        cloud_init_tmpf.write_all(cloudinit.as_bytes())?;
        cloud_init_tmpf.flush()?;
        // SAFETY: should be utf-8
        let cloud_init_tmpf = cloud_init_tmpf.path().to_str().unwrap();
        vinstall.arg(format!("--cloud-init=user-data={}", cloud_init_tmpf));

        // Pass through user-provided args
        vinstall.args(self.vinstarg);
        println!("+ {}", vinstall.to_string_pretty());
        // Drop listener at the last moment to reduce race window
        drop(port);
        vinstall
            .run()
            .map_err(|e| eyre!("Failed to run virt-install: {e}"))?;
        Ok(())
    }
}
