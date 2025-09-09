use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::run_ephemeral::RunEphemeralOpts;

#[derive(Parser)]
pub struct ContainerEntrypointOpts {
    #[command(subcommand)]
    pub command: ContainerCommands,
}

#[derive(Subcommand)]
pub enum ContainerCommands {
    /// Run ephemeral VM (what run-ephemeral-impl does today)
    RunEphemeral,

    /// Run unified install-and-boot workflow
    RunFromInstall,

    /// SSH to VM from container
    Ssh(SshOpts),
}

#[derive(Parser)]
pub struct SshOpts {
    /// SSH arguments  
    #[clap(allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Configuration passed via BCK_CONFIG environment variable
#[derive(Serialize, Deserialize)]
pub struct ContainerConfig {
    pub memory_mb: u32,
    pub vcpus: u32,
    pub console: bool,
    pub extra_args: Option<String>,
    // Future: SSH config, etc.
}

pub fn run_ephemeral_in_container() -> Result<()> {
    // Parse BCK_CONFIG from environment
    let config_json = std::env::var("BCK_CONFIG")?;
    let opts: RunEphemeralOpts = serde_json::from_str(&config_json)?;

    // Call existing run_impl
    crate::run_ephemeral::run_impl(opts)
}

pub fn ssh_to_vm(opts: SshOpts) -> Result<()> {
    debug!("SSH to VM with args: {:?}", opts.args);

    // SSH implementation
    // Default to root@10.0.2.15 (QEMU user networking)
    let mut cmd = std::process::Command::new("ssh");

    // Check if SSH key exists
    if std::path::Path::new("/tmp/ssh").exists() {
        cmd.args(["-i", "/tmp/ssh"]);
    }

    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    cmd.args(["-o", "UserKnownHostsFile=/dev/null"]);
    cmd.args(["-o", "LogLevel=ERROR"]); // Reduce SSH verbosity

    // If no host specified in args, use default
    if !opts.args.iter().any(|arg| arg.contains("@")) {
        cmd.arg("root@10.0.2.15");
    }

    // Add any additional arguments
    if !opts.args.is_empty() && !opts.args.iter().any(|arg| arg.contains("@")) {
        cmd.arg("--");
    }
    cmd.args(&opts.args);

    let status = cmd.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Run QEMU from disk within container - variant of run_ephemeral::run_impl
fn run_disk_impl(config: ContainerConfig) -> Result<()> {
    use crate::qemu;
    use color_eyre::eyre::eyre;
    use std::fs;
    use std::path::Path;

    debug!("Running QEMU disk boot implementation inside container");

    // Check if we're in debug mode
    let debug_mode = std::env::var("DEBUG_MODE").unwrap_or_default() == "true";

    // For disk boot, we don't need kernel/initramfs from container image
    // Instead, we boot directly from the disk file

    // Verify KVM access
    if !Path::new("/dev/kvm").exists() || !fs::File::open("/dev/kvm").is_ok() {
        return Err(eyre!("KVM device not accessible"));
    }

    // Find the boot disk file
    let disk_files_dir = Path::new("/run/disk-files");
    let boot_disk_path = if disk_files_dir.exists() {
        // Look for the bootdisk file
        let bootdisk_path = disk_files_dir.join("bootdisk");
        if bootdisk_path.exists() {
            Some(bootdisk_path)
        } else {
            // Fall back to first disk file found
            fs::read_dir(disk_files_dir)?
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .find(|path| path.is_file())
        }
    } else {
        None
    };

    let boot_disk_path =
        boot_disk_path.ok_or_else(|| eyre!("No boot disk found in /run/disk-files"))?;

    debug!("Found boot disk at: {:?}", boot_disk_path);

    // Process host mounts and prepare virtiofsd instances (same as ephemeral)
    // TODO: This is duplicated from run_ephemeral::run_impl - should be refactored

    // For now, implement a minimal disk boot
    if debug_mode {
        debug!("Debug mode enabled - dropping to shell instead of running QEMU");
        let mut cmd = std::process::Command::new("bash");
        cmd.arg("-i");
        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    // Create QEMU configuration for disk boot
    let mut qemu_config = qemu::QemuConfig::new_disk_boot(
        config.memory_mb,
        config.vcpus,
        boot_disk_path.to_string_lossy().to_string(),
    );

    // Enable console if requested
    if config.console {
        qemu_config.set_console(true);
    }

    // Enable SSH access with port forwarding
    qemu_config.enable_ssh_access(Some(2222));

    // Check for SSH key and inject via SMBIOS (required for disk boot)
    if std::path::Path::new("/tmp/ssh.pub").exists() {
        let public_key = std::fs::read_to_string("/tmp/ssh.pub")
            .map_err(|e| eyre!("Failed to read SSH public key: {}", e))?;
        let public_key = public_key.trim();
        debug!("Injecting SSH public key via SMBIOS for root user using smbios_cred_for_root_ssh");

        // Use smbios_cred_for_root_ssh to create the proper SMBIOS credential
        let smbios_credential = crate::sshcred::smbios_cred_for_root_ssh(public_key)?;
        qemu_config.add_smbios_credential(smbios_credential.clone());

        debug!(
            "Added SMBIOS credential for SSH root access: {}",
            smbios_credential
        );
    } else {
        debug!("No SSH public key found at /tmp/ssh.pub, SSH access will not work");
    }

    // Create systemd log file for debugging - matches run_ephemeral pattern
    let log_path = std::path::Path::new("/run/systemd-guest.txt");
    let logf = std::fs::File::create(log_path).map_err(|e| {
        eyre!(
            "Failed to create systemd log file at {}: {}",
            log_path.display(),
            e
        )
    })?;
    qemu_config.systemd_notify = Some(logf);

    // Parse configuration for additional disks
    let run_config_json = std::env::var("BCK_RUN_FROM_INSTALL_CONFIG").ok();

    let additional_disks: Vec<String> = if let Some(ref config_str) = run_config_json {
        match serde_json::from_str::<serde_json::Value>(config_str) {
            Ok(config) => config
                .get("additional_disks")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    debug!(
        "Found {} additional disks from config",
        additional_disks.len()
    );

    // TODO: Virtiofs mounts are not supported in run-from-install Phase 2
    // Unlike run_ephemeral which can inject systemd mount units into the container image,
    // the disk boot phase boots from a pre-installed bootc system that cannot auto-mount
    // virtiofs filesystems. The virtiofs devices would be available but require manual
    // mounting inside the guest (e.g., mount -t virtiofs tag /mount/point).
    // This is a fundamental limitation of the disk boot approach vs ephemeral boot.

    // Parse and add additional disks
    for disk_spec in &additional_disks {
        let parts: Vec<&str> = disk_spec.split(':').collect();
        if parts.len() != 2 {
            debug!("Ignoring invalid additional disk format: {}", disk_spec);
            continue;
        }
        let disk_path = std::path::Path::new(parts[0]);
        let disk_name = parts[1];

        if disk_path.exists() && disk_path.is_file() {
            qemu_config.add_virtio_blk_device(
                disk_path.to_string_lossy().to_string(),
                disk_name.to_string(),
            );
            debug!(
                "Added additional disk: {} as {}",
                disk_path.display(),
                disk_name
            );
        } else {
            debug!("Additional disk not found or not a file: {}", parts[0]);
        }
    }

    debug!("Starting QEMU disk boot with systemd debugging enabled");

    // Spawn QEMU with vsock debugging
    let mut child = qemu::RunningQemu::spawn(qemu_config)?;

    // Verify SMBIOS credential is present in QEMU process (sanity check)
    if std::path::Path::new("/tmp/ssh.pub").exists() {
        // Give QEMU a moment to start
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Check if QEMU process has the SMBIOS credential
        if let Ok(output) = std::process::Command::new("ps").args(["aux"]).output() {
            let ps_output = String::from_utf8_lossy(&output.stdout);
            if ps_output.contains("io.systemd.credential")
                && ps_output.contains("ssh.authorized_keys.root")
            {
                debug!("✓ VERIFIED: QEMU process contains SMBIOS SSH credential");
            } else {
                debug!("⚠ WARNING: QEMU process may not have SMBIOS SSH credential - SSH might not work");
                debug!("PS output for QEMU processes:");
                for line in ps_output.lines() {
                    if line.contains("qemu") {
                        debug!("  {}", line);
                    }
                }
            }
        }
    }

    child.wait()?;
    Ok(())
}

pub fn run_from_install_in_container() -> Result<()> {
    use serde_json::Value;

    debug!("Running unified install-and-boot workflow inside container");

    // Parse enhanced config from BCK_RUN_FROM_INSTALL_CONFIG environment variable
    // This contains the source_image and run_from_install specific configuration
    let config_json = std::env::var("BCK_RUN_FROM_INSTALL_CONFIG").unwrap_or_else(|_| {
        debug!("BCK_RUN_FROM_INSTALL_CONFIG not found, falling back to BCK_CONFIG");
        std::env::var("BCK_CONFIG").unwrap_or_else(|_| {
            r#"{"memory_mb": 2048, "vcpus": 2, "console": false, "extra_args": null}"#.to_string()
        })
    });

    debug!("Using config: {}", config_json);
    let config: Value = serde_json::from_str(&config_json)?;

    println!("Phase 1: Installing to internal disk...");
    debug!("Phase 1: Using run_install to create disk image");

    // Phase 1: Use the existing run_install code that already works
    let disk_path = "/tmp/bootc-installed.img";

    // Get source image and configuration from BCK_CONFIG
    let source_image = config
        .get("source_image")
        .and_then(|s| s.as_str())
        .ok_or_else(|| color_eyre::eyre::eyre!("No source_image in config"))?;

    let root_size = config
        .get("run_from_install")
        .and_then(|c| c.get("root_size"))
        .and_then(|r| r.as_str());

    let filesystem = config
        .get("run_from_install")
        .and_then(|c| c.get("filesystem"))
        .and_then(|f| f.as_str())
        .unwrap_or("ext4");

    debug!(
        "Phase 1: Installing {} to {} with filesystem {}",
        source_image, disk_path, filesystem
    );

    // Phase 1 (installation) does NOT need SSH keys - it's just creating the disk image
    debug!("Phase 1: SSH is not needed for installation phase");

    // Create RunInstallOpts using the same approach as the main run_from_install function
    // But use the mounted container storage path instead of auto-detection
    let storage_path = std::path::PathBuf::from("/run/host-mounts/hoststorage");

    let install_opts = crate::run_install::RunInstallOpts {
        source_image: source_image.to_string(),
        target_disk: std::path::PathBuf::from(disk_path),
        install: crate::install_options::InstallOptions {
            filesystem: Some(filesystem.to_string()),
            root_size: root_size.map(|s| s.to_string()),
            storage_path: Some(storage_path), // Use the mounted storage path
        },
        disk_size: None,
        common: crate::run_ephemeral::CommonVmOpts {
            memory: None,
            vcpus: None,
            kernel_args: Vec::new(), // No SSH kernel args needed for installation
            net: None,
            console: false,
            debug: false,
            virtio_serial_out: Vec::new(),
            execute: Default::default(),
            ssh_keygen: false,
            ssh_user: "root".to_string(),
            ssh_identity: None,
        },
    };

    debug!(
        "Phase 1: Calling run_install::run with opts: {:?}",
        install_opts
    );

    // Run the installation using the existing working code
    crate::run_install::run(install_opts)?;

    println!("✓ Phase 1 completed: Installation finished");
    debug!("Phase 1: run_install completed successfully");

    // Phase 2: Boot from the installed disk using QEMU
    println!("Phase 2: Booting from installed disk...");
    debug!("Phase 2: Starting QEMU boot from installed disk");

    // Extract SSH configuration for Phase 2 disk VM
    let ssh_keygen = config
        .get("ssh_keygen")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ssh_identity = config.get("ssh_identity").and_then(|v| v.as_str());
    let ssh_user = config
        .get("ssh_user")
        .and_then(|v| v.as_str())
        .unwrap_or("root");
    let container_name = config.get("container_name").and_then(|v| v.as_str());

    // Generate SSH key for the disk VM if requested
    let _ssh_public_key = if ssh_keygen || ssh_identity.is_some() {
        if let Some(identity_path) = ssh_identity {
            // Use existing SSH key
            debug!("Phase 2: Using existing SSH identity: {}", identity_path);
            let public_key = crate::ssh::read_public_key(&std::path::Path::new(identity_path))?;
            Some(public_key)
        } else {
            // Generate new SSH key inside container for the disk VM
            debug!("Phase 2: Generating SSH keypair for disk VM");
            let vm_id = crate::ssh::generate_vm_id();
            let cache_dir = crate::ssh::get_vm_cache_dir(&vm_id)?;
            let key_pair = crate::ssh::generate_ssh_keypair(&cache_dir, "ssh_key")?;
            debug!(
                "Phase 2: Generated SSH keypair at {}",
                key_pair.private_key_path.display()
            );

            // Store SSH config for bootc-kit ssh access
            let ssh_config = crate::ssh::VmSshConfig {
                vm_id: vm_id.clone(),
                ssh_key_path: key_pair.private_key_path.clone(),
                ssh_user: ssh_user.to_string(),
                container_name: container_name.map(|s| s.to_string()),
            };
            crate::ssh::save_vm_config(&ssh_config)?;
            debug!("Phase 2: Saved SSH config for bootc-kit ssh access");

            // Write public key to expected location for SMBIOS injection
            std::fs::write("/tmp/ssh.pub", &key_pair.public_key_content)
                .map_err(|e| eyre!("Failed to write SSH public key to /tmp/ssh.pub: {}", e))?;
            debug!("Phase 2: Wrote SSH public key to /tmp/ssh.pub");

            Some(key_pair.public_key_content)
        }
    } else {
        debug!("Phase 2: No SSH key generation requested");
        None
    };

    let boot_config = ContainerConfig {
        memory_mb: config
            .get("memory_mb")
            .and_then(|m| m.as_u64())
            .unwrap_or(2048) as u32,
        vcpus: config.get("vcpus").and_then(|v| v.as_u64()).unwrap_or(2) as u32,
        console: config
            .get("console")
            .and_then(|c| c.as_bool())
            .unwrap_or(false),
        extra_args: None, // No extra args for boot phase
    };

    // Create a symlink for the disk boot logic to find
    debug!("Phase 2: Setting up disk files for boot");
    std::fs::create_dir_all("/run/disk-files")?;
    if std::path::Path::new("/run/disk-files/bootdisk").exists() {
        std::fs::remove_file("/run/disk-files/bootdisk")?;
    }
    std::os::unix::fs::symlink(disk_path, "/run/disk-files/bootdisk")?;

    // Run Phase 2 using existing disk boot implementation
    debug!("Phase 2: Starting QEMU with disk boot");
    run_disk_impl(boot_config)?;

    println!("✓ Phase 2 completed: Booted from installed disk");
    debug!("run_from_install_in_container completed successfully");
    Ok(())
}

pub fn run(opts: ContainerEntrypointOpts) -> Result<()> {
    match opts.command {
        ContainerCommands::RunEphemeral => run_ephemeral_in_container(),
        ContainerCommands::RunFromInstall => run_from_install_in_container(),
        ContainerCommands::Ssh(ssh_opts) => ssh_to_vm(ssh_opts),
    }
}
