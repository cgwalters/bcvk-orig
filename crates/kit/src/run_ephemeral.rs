use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;

use clap::Parser;
use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use itertools::Itertools;
use rustix::path::Arg;
use tracing::{info, instrument};

#[derive(Parser, Debug)]
pub struct RunEphemeralOpts {
    /// Container image to run as VM
    pub image: String,

    /// Memory in MiB
    #[clap(long, default_value_t = 2048)]
    pub memory: u32,

    /// Number of vCPUs
    #[clap(long, default_value_t = 2)]
    pub vcpus: u32,

    /// Additional kernel command line arguments
    #[clap(long = "karg")]
    pub kernel_args: Vec<String>,

    #[clap(long, default_value = "none")]
    pub net: String,

    /// Disable console output to terminal
    #[clap(long)]
    pub no_console: bool,

    /// Enable debug mode (drop into shell instead of running QEMU)
    #[clap(long)]
    pub debug: bool,
}

#[derive(Parser, Debug)]
pub struct RunEphemeralImplOpts {
    /// Memory in MiB
    #[clap(long)]
    pub memory: u32,

    /// Number of vCPUs
    #[clap(long)]
    pub vcpus: u32,

    /// Extra kernel arguments
    #[clap(long)]
    pub extra_args: Option<String>,

    /// Enable console output
    #[clap(long)]
    pub console: bool,
}

#[instrument]
fn run_qemu_in_container(opts: &RunEphemeralOpts) -> Result<std::process::ExitStatus> {
    info!("Running QEMU inside hybrid container for {}", opts.image);

    let script = include_str!("../scripts/entrypoint.sh");

    let td = tempfile::tempdir()?;
    let td = td.path().to_str().unwrap();

    let entrypoint_path = &format!("{td}/entrypoint");
    {
        let f = File::create(entrypoint_path)?;
        let mut f = BufWriter::new(f);
        f.write_all(script.as_bytes())?;
        use std::{fs::Permissions, os::unix::fs::PermissionsExt};
        let f = f.into_inner()?;
        let perms = Permissions::from_mode(0o755);
        f.set_permissions(perms)?;
    }

    let extra_args = opts.kernel_args.iter().map(|s| s.as_str()).join(" ");

    let self_exe = std::env::current_exe()?;
    let self_exe = self_exe.as_str()?;

    // Run the container with the setup script
    let mut cmd = Command::new("podman");
    cmd.arg("run");
    cmd.arg(format!("--net={}", opts.net.as_str()));
    cmd.args([
        "--rm",
        "-it", // Interactive for console passthrough
        // Needed to create nested containers (mountns, etc). Note when running
        // with userns (podman unpriv default) this is totally safe. TODO:
        // Default to enabling userns when running rootful.
        "--cap-add=all",
        // We mount the host /usr (though just *read-only*) but to do that we need to
        // disable default SELinux confinement
        "--security-opt=label=disable",
        // Also needed for nested containers
        "--security-opt=seccomp=unconfined",
        "--security-opt=unmask=/proc/*",
        // This is a general hardening thing to do when running privileged
        "-v",
        "/sys:/sys:ro",
        "--device=/dev/kvm",
        "-v",
        "/usr:/run/hostusr:ro", // Bind mount host /usr as read-only
        "-v",
        &format!("{entrypoint_path}:/run/entrypoint"),
        "-v",
        &format!("{self_exe}:/run/selfexe:ro"),
        // And bind mount in the pristine image (without any mounts on top)
        // that we'll use as a mount source for virtiofs.
        &format!(
            "--mount=type=image,source={},target=/run/source-image",
            opts.image.as_str()
        ),
    ]);

    // Set debug mode environment variable if requested
    if opts.debug {
        cmd.args(["-e", "DEBUG_MODE=true"]);
        info!("Debug mode enabled - will drop into shell instead of running QEMU");
    }

    // Pass configuration as environment variables for the entrypoint script
    cmd.args([
        "-e",
        &format!("BOOTC_MEMORY={}", opts.memory),
        "-e",
        &format!("BOOTC_VCPUS={}", opts.vcpus),
    ]);

    if !extra_args.is_empty() {
        cmd.args(["-e", &format!("BOOTC_EXTRA_ARGS={}", extra_args)]);
    }

    if !opts.no_console {
        cmd.args(["-e", "BOOTC_CONSOLE=1"]);
    }

    let status = cmd
        .args([&opts.image, "/run/entrypoint"])
        .status()
        .context("Failed to run QEMU in container")?;

    Ok(status)
}

#[instrument]
pub fn run(opts: RunEphemeralOpts) -> Result<()> {
    // Run QEMU inside the container with the hybrid rootfs approach
    let status = run_qemu_in_container(&opts)?;

    // QEMU may exit with non-zero status when VM powers off
    // For testing with poweroff.target, we accept exit code 1
    if !status.success() {
        if let Some(code) = status.code() {
            let kargs_str = opts.kernel_args.join(" ");
            if code == 1 && kargs_str.contains("poweroff.target") {
                info!("QEMU exited with code 1 (expected for poweroff.target)");
            } else {
                return Err(eyre!("QEMU exited with non-zero status: {}", code));
            }
        } else {
            return Err(eyre!("QEMU terminated by signal"));
        }
    }

    info!("VM terminated successfully");
    Ok(())
}

pub(crate) fn run_impl(opts: RunEphemeralImplOpts) -> Result<()> {
    use crate::qemu;
    use crate::virtiofsd;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    info!("Running QEMU implementation inside container");

    // Check if we're in debug mode
    let debug_mode = std::env::var("DEBUG_MODE").unwrap_or_default() == "true";

    // Find kernel and initramfs from the container image (not the host)
    let modules_dir = Path::new("/run/source-image/usr/lib/modules");
    let mut vmlinuz_path = None;
    let mut initramfs_path = None;

    for entry in fs::read_dir(modules_dir)? {
        let entry = entry?;
        let kernel_dir = entry.path();
        if kernel_dir.is_dir() {
            let vmlinuz = kernel_dir.join("vmlinuz");
            let initramfs = kernel_dir.join("initramfs.img");
            if vmlinuz.exists() && initramfs.exists() {
                info!("Found kernel at: {:?}", vmlinuz);
                vmlinuz_path = Some(vmlinuz);
                initramfs_path = Some(initramfs);
                break;
            }
        }
    }

    let vmlinuz_path = vmlinuz_path
        .ok_or_else(|| eyre!("No kernel found in /run/source-image/usr/lib/modules"))?;
    let initramfs_path = initramfs_path
        .ok_or_else(|| eyre!("No initramfs found in /run/source-image/usr/lib/modules"))?;

    // Verify KVM access
    if !Path::new("/dev/kvm").exists() || !fs::File::open("/dev/kvm").is_ok() {
        return Err(eyre!("KVM device not accessible"));
    }

    // Create QEMU mount points
    fs::create_dir_all("/run/qemu")?;
    let kernel_mount = "/run/qemu/kernel";
    let initramfs_mount = "/run/qemu/initramfs";
    fs::File::create(&kernel_mount)?;
    fs::File::create(&initramfs_mount)?;

    // Bind mount kernel and initramfs
    let mut mount_cmd = Command::new("mount");
    mount_cmd.args([
        "--bind",
        "-o",
        "ro",
        vmlinuz_path.to_str().unwrap(),
        &kernel_mount,
    ]);
    let status = mount_cmd.status().context("Failed to bind mount kernel")?;
    if !status.success() {
        return Err(eyre!("Failed to bind mount kernel"));
    }

    let mut mount_cmd = Command::new("mount");
    mount_cmd.args([
        "--bind",
        "-o",
        "ro",
        initramfs_path.to_str().unwrap(),
        &initramfs_mount,
    ]);
    let status = mount_cmd
        .status()
        .context("Failed to bind mount initramfs")?;
    if !status.success() {
        return Err(eyre!("Failed to bind mount initramfs"));
    }

    // Start virtiofsd in background
    let virtiofsd_config = virtiofsd::VirtiofsdConfig::default();
    let mut virtiofsd = virtiofsd::spawn_virtiofsd(&virtiofsd_config)?;

    // Wait for socket to be created
    std::thread::sleep(Duration::from_secs(2));

    if debug_mode {
        info!("=== DEBUG MODE: Dropping into bash shell ===");
        info!("Environment setup complete. You can:");
        info!("- Inspect /run/tmproot (the hybrid rootfs)");
        info!("- Check virtiofsd socket at /run/inner-shared/virtiofs.sock");
        info!("- Exit with 'exit' to terminate");

        let status = Command::new("bash")
            .status()
            .context("Failed to start debug shell")?;

        // Clean up virtiofsd
        virtiofsd.kill().ok();

        if !status.success() {
            return Err(eyre!("Debug shell exited with non-zero status"));
        }
    } else {
        // Build kernel command line
        let mut kernel_cmdline = vec![
            "rootfstype=virtiofs".to_string(),
            "root=rootfs".to_string(),
            "selinux=0".to_string(),
            "systemd.volatile=overlay".to_string(),
        ];

        if opts.console {
            kernel_cmdline.push("console=ttyS0".to_string());
        }

        if let Some(ref extra_args) = opts.extra_args {
            kernel_cmdline.push(extra_args.clone());
        }

        // Configure and start QEMU
        let qemu_config = qemu::QemuConfig {
            memory_mb: opts.memory,
            vcpus: opts.vcpus,
            kernel_path: "/run/qemu/kernel".to_string(),
            initramfs_path: "/run/qemu/initramfs".to_string(),
            virtiofs_socket: virtiofsd_config.socket_path.clone(),
            kernel_cmdline,
            enable_console: opts.console,
        };

        info!("Starting QEMU");
        let mut qemu = qemu::spawn_qemu(&qemu_config)?;

        // Wait for QEMU to finish
        let status = qemu.wait().context("Failed to wait for QEMU")?;

        // Clean up virtiofsd
        virtiofsd.kill().ok();

        // QEMU may exit with non-zero status when VM powers off
        // For testing with poweroff.target, we accept exit code 1
        if !status.success() {
            if let Some(code) = status.code() {
                if code == 1
                    && opts
                        .extra_args
                        .as_ref()
                        .map_or(false, |args| args.contains("poweroff.target"))
                {
                    info!("QEMU exited with code 1 (expected for poweroff.target)");
                } else {
                    return Err(eyre!("QEMU exited with non-zero status: {}", code));
                }
            } else {
                return Err(eyre!("QEMU terminated by signal"));
            }
        }
    }

    Ok(())
}
