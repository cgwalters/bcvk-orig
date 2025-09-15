//! Ephemeral VM execution using hybrid container-VM approach.
//!
//! This module implements a sophisticated architecture for running container images as
//! ephemeral VMs by orchestrating a multi-stage execution flow through privileged
//! containers, namespace isolation, and VirtioFS filesystem sharing.
//!
//! # Architecture Overview
//!
//! The system uses a "hybrid container-VM" approach that runs QEMU inside privileged
//! Podman containers with KVM access. This combines container isolation with full
//! kernel VM capabilities.
//!
//! ## Execution Flow
//!
//! The execution follows this chain:
//! 1. **Host Process**: `bcvk run-ephemeral` invoked on host
//! 2. **Container Launch**: Podman privileged container with KVM and host mounts
//! 3. **Namespace Setup**: bwrap creates isolated namespace with hybrid rootfs  
//! 4. **Binary Re-execution**: Same binary re-executes with `container-entrypoint`
//! 5. **VM Launch**: QEMU starts with VirtioFS root and additional mounts
//!
//! ## Key Components
//!
//! ### Phase 1: Container Setup (`run_qemu_in_container`)
//! - Runs on the host system
//! - Serializes CLI options to JSON via `BCK_CONFIG` environment variable
//! - Mounts critical resources into container:
//!   - `/run/selfexe`: The bcvk binary itself (for re-execution)
//!   - `/run/source-image`: Target container image via `--mount=type=image`
//!   - `/run/hostusr`: Host `/usr` directory (read-only, for QEMU/tools)
//!   - `/var/lib/bcvk/entrypoint`: Embedded entrypoint.sh script
//! - Handles real-time output streaming for `--execute` commands
//!
//! ### Phase 2: Hybrid Rootfs Creation (entrypoint.sh)
//! The entrypoint script creates a hybrid root filesystem at `/run/tmproot`:
//! ```text
//! /run/tmproot/
//! ├── usr/       → bind mount to /run/hostusr (host binaries)
//! ├── bin/       → symlink to usr/bin
//! ├── lib/       → symlink to usr/lib
//! └── [other dirs created empty for container compatibility]
//! ```
//!
//! ### Phase 3: Namespace Isolation (bwrap)
//! Uses bubblewrap to create isolated namespace:
//! - New mount namespace with `/run/tmproot` as root
//! - Shared `/run/inner-shared` for virtiofsd socket communication
//! - Proper `/proc`, `/dev`, `/tmp` mounts
//! - Re-executes binary: `bwrap ... -- /run/selfexe container-entrypoint`
//!
//! ### Phase 4: VM Execution (`run_impl`)
//! - Runs inside the container after namespace setup
//! - Extracts kernel/initramfs from container image
//! - Spawns virtiofsd daemons for filesystem sharing:
//!   - Main daemon: shares `/run/source-image` as VM root
//!   - Additional daemons: one per host mount (`--bind`/`--ro-bind`)
//! - Generates systemd `.mount` units for virtiofs mounts
//! - Configures and launches QEMU with VirtioFS root
//!
//! ## VirtioFS Architecture
//!
//! The system uses VirtioFS for high-performance filesystem sharing:
//! - **Root FS**: Container image mounted via main virtiofsd at `/run/inner-shared/virtiofs.sock`
//! - **Host Mounts**: Separate virtiofsd per mount at `/run/inner-shared/virtiofs-<name>.sock`
//! - **VM Access**: Mounts appear at `/run/virtiofs-mnt-<name>` via systemd units
//!
//! ## Command Execution (`--execute`)
//!
//! For running commands inside the VM:
//! 1. Creates systemd services (`bootc-execute.service`, `bootc-execute-finish.service`)
//! 2. Uses VirtioSerial devices for output (`execute`) and status (`executestatus`)
//! 3. Streams output in real-time via monitoring thread on host
//! 4. Captures exit codes via systemd service status
//!
//! ## Security Model
//!
//! - **Privileged Container**: Required for KVM and namespace operations
//! - **Read-only Host Access**: Host `/usr` mounted read-only
//! - **SELinux**: Disabled within container only (`--security-opt=label=disable`)
//! - **Network Isolation**: Default "none" unless explicitly configured
//! - **VirtioFS Sandboxing**: Relies on VM isolation for security
//!
//! ## Configuration Passing
//!
//! All CLI options are preserved through the execution chain via JSON serialization:
//! - Host serializes `RunEphemeralOpts` to `BCK_CONFIG` environment variable
//! - Container entrypoint deserializes and re-applies all settings
//! - Ensures perfect fidelity of user options across process boundaries

use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::unix::process::CommandExt;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use rustix::path::Arg;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tracing::debug;

/// Default memory size in MB
pub const DEFAULT_MEMORY_MB: u32 = 2048;

/// Default memory size as string for clap defaults (in MB)
pub const DEFAULT_MEMORY_STR: &str = "2048";

/// Default memory size as string for user-facing defaults (in GB)
pub const DEFAULT_MEMORY_USER_STR: &str = "2G";

const ENTRYPOINT: &str = "/var/lib/bcvk/entrypoint";

/// Get default vCPU count (number of available processors, or 2 as fallback)
pub fn default_vcpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(2)
}

use crate::{podman, systemd, utils, CONTAINER_STATEDIR};

/// Common container lifecycle options for podman commands.
#[derive(Parser, Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommonPodmanOptions {
    #[clap(
        short = 't',
        long = "tty",
        help = "Allocate a pseudo-TTY for container"
    )]
    pub tty: bool,

    #[clap(
        short = 'i',
        long = "interactive",
        help = "Keep STDIN open for container"
    )]
    pub interactive: bool,

    #[clap(short = 'd', long = "detach", help = "Run container in background")]
    pub detach: bool,

    #[clap(long = "rm", help = "Automatically remove container when it exits")]
    pub rm: bool,

    #[clap(long = "name", help = "Assign a name to the container")]
    pub name: Option<String>,

    #[clap(
        long = "label",
        help = "Add metadata to the container in key=value form"
    )]
    pub label: Vec<String>,
}

/// Common VM configuration options for hardware, networking, and features.
#[derive(Parser, Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommonVmOpts {
    #[clap(
        long,
        default_value = DEFAULT_MEMORY_STR,
        help = "Memory size (e.g. 2G, 1024M, 512m, or plain number for MB)"
    )]
    pub memory: Option<String>,

    #[clap(long, default_value_t = default_vcpus(), help = "Number of vCPUs")]
    pub vcpus: u32,

    #[clap(long = "karg", help = "Additional kernel command line arguments")]
    pub kernel_args: Vec<String>,

    #[clap(
        long,
        help = "Network configuration (none, user, bridge=name) [default: none]"
    )]
    pub net: Option<String>,

    #[clap(long, help = "Enable console output to terminal for debugging")]
    pub console: bool,

    #[clap(
        long,
        help = "Enable debug mode (drop to shell instead of running QEMU)"
    )]
    pub debug: bool,

    #[clap(
        long = "virtio-serial-out",
        value_name = "NAME:FILE",
        help = "Add virtio-serial device with output to file (format: name:/path/to/file)"
    )]
    pub virtio_serial_out: Vec<String>,

    #[clap(
        long,
        help = "Execute command inside VM via systemd and capture output"
    )]
    pub execute: Vec<String>,

    #[clap(
        long,
        short = 'K',
        help = "Generate SSH keypair and inject via systemd credentials"
    )]
    pub ssh_keygen: bool,
}

impl CommonVmOpts {
    /// Parse memory specification to MB
    pub fn memory_mb(&self) -> color_eyre::Result<u32> {
        if let Some(ref mem_str) = self.memory {
            crate::utils::parse_memory_to_mb(mem_str)
        } else {
            Ok(DEFAULT_MEMORY_MB)
        }
    }

    /// Get vCPU count
    pub fn vcpus(&self) -> u32 {
        self.vcpus
    }

    /// Get network config (default: "none")
    pub fn net_string(&self) -> String {
        self.net.clone().unwrap_or_else(|| "none".to_string())
    }
}

/// Ephemeral VM options: container-style flags, host bind mounts, systemd injection.
#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
pub struct RunEphemeralOpts {
    #[clap(help = "Container image to run as ephemeral VM")]
    pub image: String,

    #[clap(flatten)]
    pub common: CommonVmOpts,

    #[clap(flatten)]
    pub podman: CommonPodmanOptions,

    #[clap(
        long = "bind",
        value_name = "HOST_PATH[:NAME]",
        help = "Bind mount host directory (RW) at /run/virtiofs-mnt-<name>"
    )]
    pub bind_mounts: Vec<String>,

    #[clap(
        long = "ro-bind",
        value_name = "HOST_PATH[:NAME]",
        help = "Bind mount host directory (RO) at /run/virtiofs-mnt-<name>"
    )]
    pub ro_bind_mounts: Vec<String>,

    #[clap(
        long = "systemd-units",
        help = "Directory with systemd units to inject (expects system/ subdirectory)"
    )]
    pub systemd_units_dir: Option<String>,

    #[clap(
        long = "log-cmdline",
        help = "Log full podman command before execution"
    )]
    pub log_cmdline: bool,

    #[clap(
        long = "bind-storage-ro",
        help = "Mount host container storage (RO) at /run/virtiofs-mnt-hoststorage"
    )]
    pub bind_storage_ro: bool,

    #[clap(
        long = "mount-disk-file",
        value_name = "FILE[:NAME]",
        help = "Mount disk file as virtio-blk device at /dev/disk/by-id/virtio-<name>"
    )]
    pub mount_disk_files: Vec<String>,
}

/// Launch privileged container with QEMU+KVM for ephemeral VM, spawning as subprocess.
/// Returns the container ID instead of executing the command.
pub fn run_detached(opts: RunEphemeralOpts) -> Result<String> {
    let (mut cmd, temp_dir) = prepare_run_command_with_temp(opts)?;

    // Leak the tempdir to keep it alive for the entire container lifetime
    std::mem::forget(temp_dir);

    let output = cmd.output().context("Failed to execute podman command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!("Podman command failed: {}", stderr));
    }

    // Return the container ID from stdout
    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(container_id)
}

/// Launch privileged container with QEMU+KVM for ephemeral VM.
pub fn run(opts: RunEphemeralOpts) -> Result<()> {
    let (mut cmd, _temp_dir) = prepare_run_command_with_temp(opts)?;
    // Keep _temp_dir alive until exec replaces our process
    // At this point our process is replaced by `podman`, we are just a wrapper for creating
    // a container image and nothing else lives past that event.
    return Err(cmd.exec()).context("execve");
}

/// Launch privileged container with QEMU+KVM for ephemeral VM and wait for completion.
/// Unlike `run()`, this function waits for completion instead of using exec(), making it suitable
/// for programmatic use where the caller needs to capture output and exit codes.
pub fn run_synchronous(opts: RunEphemeralOpts) -> Result<()> {
    let (mut cmd, temp_dir) = prepare_run_command_with_temp(opts)?;
    // Keep temp_dir alive until command completes

    // Use the same approach as run_detached but wait for completion instead of detaching
    let output = cmd.output().context("Failed to execute podman command")?;

    // Forward the output to our stdout/stderr so it appears in logs and tests
    if !output.stdout.is_empty() {
        std::io::Write::write_all(&mut std::io::stdout(), &output.stdout)?;
        std::io::Write::flush(&mut std::io::stdout())?;
    }
    if !output.stderr.is_empty() {
        std::io::Write::write_all(&mut std::io::stderr(), &output.stderr)?;
        std::io::Write::flush(&mut std::io::stderr())?;
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Podman command failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        ));
    }

    // Explicitly drop temp_dir after successful completion
    drop(temp_dir);
    Ok(())
}

fn prepare_run_command_with_temp(
    opts: RunEphemeralOpts,
) -> Result<(std::process::Command, tempfile::TempDir)> {
    debug!("Running QEMU inside hybrid container for {}", opts.image);

    let script = include_str!("../scripts/entrypoint.sh");

    let td = tempfile::tempdir()?;
    let td_path = td.path().to_str().unwrap();

    let entrypoint_path = &format!("{}/entrypoint", td_path);
    {
        let f = File::create(entrypoint_path)?;
        let mut f = BufWriter::new(f);
        f.write_all(script.as_bytes())?;
        use std::{fs::Permissions, os::unix::fs::PermissionsExt};
        let f = f.into_inner()?;
        let perms = Permissions::from_mode(0o755);
        f.set_permissions(perms)?;
    }

    let self_exe = std::env::current_exe()?;
    let self_exe = self_exe.as_str()?;

    // Process disk files and create them if needed
    let processed_disk_files = process_disk_files(&opts.mount_disk_files, &opts.image)?;

    // Parse mount arguments (both bind and ro-bind)
    let mut host_mounts = Vec::new();

    // Add container storage mount if requested
    if opts.bind_storage_ro {
        let storage_path = utils::detect_container_storage_path().context(
            "Failed to detect container storage path. Use --ro-bind to specify manually.",
        )?;
        utils::validate_container_storage_path(&storage_path)
            .context("Container storage validation failed")?;

        debug!(
            "Adding container storage from {} as hoststorage mount",
            storage_path
        );
        host_mounts.push((storage_path.to_string(), "hoststorage".to_string(), true));
        // true = read-only
    }

    // Parse writable bind mounts
    for mount_spec in &opts.bind_mounts {
        let (host_path, mount_name) = if let Some((path, name)) = mount_spec.split_once(':') {
            (path.to_string(), name.to_string())
        } else {
            let path = mount_spec.clone();
            let name = Utf8Path::new(&path)
                .file_name()
                .unwrap_or("mount")
                .to_string();
            (path, name)
        };
        host_mounts.push((host_path, mount_name, false)); // false = writable
    }

    // Parse read-only bind mounts
    for mount_spec in &opts.ro_bind_mounts {
        let (host_path, mount_name) = if let Some((path, name)) = mount_spec.split_once(':') {
            (path.to_string(), name.to_string())
        } else {
            let path = mount_spec.clone();
            let name = Utf8Path::new(&path)
                .file_name()
                .unwrap_or("mount")
                .to_string();
            (path, name)
        };
        host_mounts.push((host_path, mount_name, true)); // true = read-only
    }

    // Run the container with the setup script
    let mut cmd = Command::new("podman");
    cmd.arg("run");
    // We always have a label
    cmd.arg("--label=bcvk.ephemeral=1");
    for label in opts.podman.label.iter() {
        cmd.arg(format!("--label={label}"));
    }
    cmd.arg(format!("--net={}", opts.common.net_string().as_str()));

    // Add container name if specified
    if let Some(ref name) = opts.podman.name {
        cmd.args(["--name", name]);
    }

    // Add --rm flag based on user input (default: true)
    if opts.podman.rm {
        cmd.arg("--rm");
    }

    // Add -t, -i, -d flags based on user input (mirror podman behavior)
    if opts.podman.tty {
        cmd.arg("-t");
    }
    if opts.podman.interactive {
        cmd.arg("-i");
    }
    if opts.podman.detach {
        cmd.arg("-d");
    }

    cmd.args([
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
        "--device=/dev/vhost-vsock",
        "-v",
        "/usr:/run/hostusr:ro", // Bind mount host /usr as read-only
        "-v",
        &format!("{}:{}", entrypoint_path, ENTRYPOINT),
        "-v",
        &format!("{self_exe}:/run/selfexe:ro"),
        // Since we run as init by default
        "--stop-signal=SIGKILL",
        // And bind mount in the pristine image (without any mounts on top)
        // that we'll use as a mount source for virtiofs. Mount as rw for testing.
        &format!(
            "--mount=type=image,source={},target=/run/source-image,rw=true",
            opts.image.as_str()
        ),
    ]);

    // Add host directory mounts to the container
    for (host_path, mount_name, is_readonly) in &host_mounts {
        let mount_spec = if *is_readonly {
            format!("{}:/run/host-mounts/{}:ro", host_path, mount_name)
        } else {
            format!("{}:/run/host-mounts/{}", host_path, mount_name)
        };
        cmd.args(["-v", &mount_spec]);
    }

    // Mount disk files into the container
    for (disk_file, disk_name) in &processed_disk_files {
        let container_disk_path = format!("/run/disk-files/{}", disk_name);
        cmd.args(["-v", &format!("{}:{}:rw", disk_file, container_disk_path)]);
    }

    // Mount systemd units directory if specified
    if let Some(ref units_dir) = opts.systemd_units_dir {
        cmd.args(["-v", &format!("{}:/run/systemd-units:ro", units_dir)]);
    }

    // Propagate this by default
    if let Some(log) = std::env::var("RUST_LOG").ok() {
        cmd.arg(format!("--env=RUST_LOG={log}"));
    }

    // Pass configuration as JSON via BCK_CONFIG environment variable
    let config = serde_json::to_string(&opts).unwrap();
    cmd.args(["-e", &format!("BCK_CONFIG={config}")]);

    // Handle --execute output files and virtio-serial devices
    let mut all_serial_devices = opts.common.virtio_serial_out.clone();
    if !opts.common.execute.is_empty() {
        // Add virtio-serial devices for execute output and status
        // These will be created inside the container at /run/execute-output/
        all_serial_devices.push("execute:/run/execute-output/execute-output.txt".to_string());
        all_serial_devices.push("executestatus:/run/execute-output/execute-status.txt".to_string());
    }

    // Pass disk files as environment variable
    if !processed_disk_files.is_empty() {
        let disk_specs = processed_disk_files
            .iter()
            .map(|(_, disk_name)| format!("/run/disk-files/{}:{}", disk_name, disk_name))
            .collect::<Vec<_>>()
            .join(",");
        cmd.args(["-e", &format!("BOOTC_DISK_FILES={}", disk_specs)]);
    }

    cmd.args([&opts.image, ENTRYPOINT]);

    // Log the full command line if requested
    if opts.log_cmdline {
        let args: Vec<String> = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        debug!("Executing: podman {}", args.join(" "));
    }

    Ok((cmd, td))
}

/// Process --mount-disk-file specs: parse file:name format, create sparse files if needed (2x image size),
/// validate only regular files, convert to absolute paths.
pub(crate) fn process_disk_files(
    disk_specs: &[String],
    image: &str,
) -> Result<Vec<(Utf8PathBuf, String)>> {
    use std::fs::File;

    let mut processed_disks = Vec::new();

    if disk_specs.is_empty() {
        return Ok(processed_disks);
    }

    // Get image size for auto-sizing new disk files (2x the image size)
    let image_size = match podman::get_image_size(image) {
        Ok(size) => size,
        Err(e) => {
            debug!(
                "Warning: Could not get image size for {}: {}. Using default size 4GB.",
                image, e
            );
            4 * 1024 * 1024 * 1024 // Default to 4GB
        }
    };
    let disk_size = image_size * 2;

    for disk_spec in disk_specs {
        let (disk_file, disk_name) = if let Some((file, name)) = disk_spec.split_once(':') {
            (file.to_string(), name.to_string())
        } else {
            (disk_spec.clone(), "output".to_string())
        };

        let disk_path = Utf8Path::new(&disk_file);

        // Security check: only accept regular files
        if disk_path.exists() {
            let metadata = disk_path
                .metadata()
                .with_context(|| format!("Failed to get metadata for disk file: {}", disk_file))?;

            if !metadata.is_file() {
                return Err(eyre!(
                    "Disk file must be a regular file, not a directory or block device: {}",
                    disk_file
                ));
            }
        } else {
            // Create sparse disk image file
            debug!(
                "Creating disk image file: {} (size: {} bytes)",
                disk_file, disk_size
            );
            let file = File::create(&disk_path)
                .with_context(|| format!("Failed to create disk file: {}", disk_file))?;

            file.set_len(disk_size)
                .with_context(|| format!("Failed to set size for disk file: {}", disk_file))?;

            debug!("Created sparse disk image: {}", disk_file);
        }

        // Convert relative paths to absolute paths for QEMU
        let absolute_disk_file = if disk_path.is_absolute() {
            disk_file.into()
        } else {
            let p = disk_path.canonicalize()?;
            Utf8PathBuf::try_from(p)?
        };

        processed_disks.push((absolute_disk_file, disk_name));
    }

    Ok(processed_disks)
}

/// Copy systemd units from /run/systemd-units/system/ to container image /etc/systemd/system/.
/// Auto-enables .mount units in local-fs.target.wants/, preserves default.target.wants/ symlinks.
fn inject_systemd_units() -> Result<()> {
    use std::fs;

    debug!("Injecting systemd units from /run/systemd-units");

    let source_units = Utf8Path::new("/run/systemd-units/system");
    if !source_units.exists() {
        debug!("No systemd units to inject at {}", source_units);
        return Ok(());
    }
    let target_units = "/run/source-image/etc/systemd/system";

    // Create target directories
    fs::create_dir_all(target_units)?;
    fs::create_dir_all(&format!("{}/default.target.wants", target_units))?;
    fs::create_dir_all(&format!("{}/local-fs.target.wants", target_units))?;

    // Copy all .service and .mount files
    for entry in fs::read_dir(source_units)? {
        let entry = entry?;
        let path = entry.path();
        let extension = path.extension().map(|ext| ext.to_string_lossy());
        if matches!(extension.as_deref(), Some("service") | Some("mount")) {
            let filename = path.file_name().unwrap().to_string_lossy();
            let target_path = format!("{}/{}", target_units, filename);
            fs::copy(&path, &target_path)?;
            debug!("Copied systemd unit: {}", filename);

            // Create symlinks for mount units to enable them
            if extension.as_deref() == Some("mount") {
                let wants_dir = format!("{}/local-fs.target.wants", target_units);
                let symlink_path = format!("{}/{}", wants_dir, filename);
                let relative_target = format!("../{}", filename);
                std::os::unix::fs::symlink(&relative_target, &symlink_path).ok();
                debug!("Enabled mount unit: {}", filename);
            }
        }
    }

    // Copy wants directory if it exists
    let source_wants = "/run/systemd-units/system/default.target.wants";
    let target_wants = &format!("{}/default.target.wants", target_units);

    if Utf8Path::new(source_wants).exists() {
        for entry in fs::read_dir(source_wants)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_symlink() || path.is_file() {
                let filename = path.file_name().unwrap().to_string_lossy();
                let target_path = format!("{}/{}", target_wants, filename);

                if path.is_symlink() {
                    let link_target = fs::read_link(&path)?;
                    let _ = fs::remove_file(&target_path); // Remove if exists
                    std::os::unix::fs::symlink(link_target, &target_path)?;
                } else {
                    fs::copy(&path, &target_path)?;
                }
                debug!("Copied systemd wants link: {}", filename);
            }
        }
    }

    debug!("Systemd unit injection complete");
    Ok(())
}

/// Parse exit code from systemd service status output
fn parse_service_exit_code(status_content: &str) -> Result<i32> {
    for line in status_content.lines() {
        if let Some(codeval) = line.strip_prefix("ExecMainStatus=") {
            let exit_code: i32 = codeval.parse().context("Parsing ExecMainStatus")?;
            return Ok(exit_code);
        }
    }
    // If no exit code found, assume success
    Ok(0)
}

/// VM execution inside container: extracts kernel/initramfs, starts virtiofsd processes,
/// generates systemd mount units, sets up command execution, launches QEMU.
pub(crate) async fn run_impl(opts: RunEphemeralOpts) -> Result<()> {
    use crate::qemu;
    use std::fs;

    debug!("Running QEMU implementation inside container");

    let systemd_version = {
        let v = std::env::var("SYSTEMD_VERSION")?;
        systemd::SystemdVersion::from_version_output(&v)?
    };

    // Find kernel and initramfs from the container image (not the host)
    let modules_dir = Utf8Path::new("/run/source-image/usr/lib/modules");
    let mut vmlinuz_path = None;
    let mut initramfs_path = None;

    for entry in fs::read_dir(modules_dir)? {
        let entry = entry?;
        let kernel_dir = entry.path();
        if kernel_dir.is_dir() {
            let vmlinuz = kernel_dir.join("vmlinuz");
            let initramfs = kernel_dir.join("initramfs.img");
            if vmlinuz.exists() && initramfs.exists() {
                debug!("Found kernel at: {:?}", vmlinuz);
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
    if !Utf8Path::new("/dev/kvm").exists() || !fs::File::open("/dev/kvm").is_ok() {
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

    // Process host mounts and prepare virtiofsd instances for each using async manager
    let mut additional_mounts = Vec::new();

    debug!(
        "Checking for host mounts directory: /run/host-mounts exists = {}",
        Utf8Path::new("/run/host-mounts").exists()
    );
    debug!(
        "Checking for systemd units directory: /run/systemd-units exists = {}",
        Utf8Path::new("/run/systemd-units").exists()
    );

    let target_unitdir = "/run/source-image/etc/systemd/system";

    if Utf8Path::new("/run/host-mounts").exists() {
        for entry in fs::read_dir("/run/host-mounts")? {
            let entry = entry?;
            let mount_name = entry.file_name();
            let mount_name_str = mount_name.to_string_lossy();
            let source_path = entry.path();
            let mount_path = format!("/run/host-mounts/{}", mount_name_str);

            // Check if this directory is mounted as read-only
            let is_readonly =
                !rustix::fs::access(&mount_path, rustix::fs::Access::WRITE_OK).is_ok();

            let mode = if is_readonly { "ro" } else { "rw" };
            debug!(
                "Setting up virtiofs mount for {} ({})",
                mount_name_str, mode
            );

            // Create virtiofs socket path and tag
            let socket_path = format!("/run/inner-shared/virtiofs-{}.sock", mount_name_str);
            let tag = format!("mount_{}", mount_name_str);

            // Store virtiofsd config to be spawned later by QEMU
            let virtiofsd_config = qemu::VirtiofsConfig {
                socket_path: socket_path.clone(),
                shared_dir: source_path.to_string_lossy().to_string(),
                cache_mode: "always".to_string(),
                sandbox: "none".to_string(),
                debug: false,
            };
            additional_mounts.push((virtiofsd_config, tag.clone()));

            // Create individual .mount unit for this virtiofs mount
            let mount_point = format!("/run/virtiofs-mnt-{}", mount_name_str);

            // Use systemd-escape to properly escape the mount path
            let escaped_path = Command::new("systemd-escape")
                .args(["-p", &mount_point])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| {
                    // Fallback if systemd-escape is not available
                    mount_point
                        .replace("/", "-")
                        .trim_start_matches('-')
                        .to_string()
                });

            let mount_unit_name = format!("{}.mount", escaped_path);
            let mount_options = if is_readonly { "ro" } else { "defaults" };

            let mount_unit_content = format!(
                r#"[Unit]
Description=Mount virtiofs {}
DefaultDependencies=no
After=systemd-remount-fs.service
Before=local-fs.target shutdown.target

[Mount]
What={}
Where={}
Type=virtiofs
Options={}

[Install]
WantedBy=local-fs.target
"#,
                mount_name_str, tag, mount_point, mount_options
            );

            let mount_unit_path = format!("{target_unitdir}/{mount_unit_name}");
            fs::write(&mount_unit_path, mount_unit_content)
                .with_context(|| format!("Failed to write mount unit to {}", mount_unit_path))?;

            // Enable the mount unit by creating symlink in local-fs.target.wants/
            let wants_dir = format!("{target_unitdir}/local-fs.target.wants");
            fs::create_dir_all(&wants_dir)?;
            let wants_link = format!("{}/{}", wants_dir, mount_unit_name);
            let relative_target = format!("../{}", mount_unit_name);
            std::os::unix::fs::symlink(&relative_target, &wants_link)?;

            // Create mount point directory in the image
            let image_mount_point = format!("/run/source-image{}", mount_point);
            fs::create_dir_all(&image_mount_point).ok();

            debug!(
                "Generated mount unit: {} (enabled in local-fs.target)",
                mount_unit_name
            );
        }
    }

    // Handle --execute: pipes will be created when adding to qemu_config later
    // No need to create files anymore as we're using pipes

    match opts.common.execute.as_slice() {
        [] => {}
        elts => {
            let wantsdir = format!("{target_unitdir}/default.target.wants");
            fs::create_dir_all(&wantsdir)?;

            let mut service_content = format!(
                r#"[Unit]
Description=Execute Script Service
Requires=dev-virtio\\x2dports-execute.device

[Service]
Type=oneshot
RemainAfterExit=yes
StandardOutput=file:/dev/virtio-ports/execute
StandardError=inherit
"#
            );
            for elt in elts {
                service_content.push_str(&format!("ExecStart={elt}\n"));
            }

            let service_finish = format!(
                r#"[Unit]
Description=Execute Script Service Completion
After=bootc-execute.service
Requires=dev-virtio\\x2dports-executestatus.device

[Service]
Type=oneshot
ExecStart=systemctl show bootc-execute
ExecStart=systemctl poweroff
StandardOutput=file:/dev/virtio-ports/executestatus
"#
            );

            let service_path = format!("{target_unitdir}/bootc-execute.service");
            fs::write(service_path, service_content)?;
            let service_path = format!("{target_unitdir}/bootc-execute-finish.service");
            fs::write(service_path, service_finish)?;

            for svc in ["bootc-execute.service", "bootc-execute-finish.service"] {
                let wants_link = format!("{wantsdir}/{svc}");
                debug!("Creating execute service symlink: {}", &wants_link);
                std::os::unix::fs::symlink(format!("../{svc}"), wants_link)?;
            }
        }
    }

    // Copy systemd units if provided (after mount units have been generated)
    // Also inject if we created mount units that need to be copied
    inject_systemd_units()?;

    // Prepare main virtiofsd config for the source image (will be spawned by QEMU)
    let mut main_virtiofsd_config = qemu::VirtiofsConfig::default();
    main_virtiofsd_config.debug = std::env::var("DEBUG_MODE").is_ok();

    std::fs::create_dir_all(CONTAINER_STATEDIR)?;

    // Configure qemu
    let mut qemu_config = crate::qemu::QemuConfig::new_direct_boot(
        opts.common.memory_mb()?,
        opts.common.vcpus(),
        "/run/qemu/kernel".to_string(),
        "/run/qemu/initramfs".to_string(),
        main_virtiofsd_config.socket_path.clone(),
    );

    // Handle SSH key generation and credential injection
    if opts.common.ssh_keygen {
        let key_pair = crate::ssh::generate_default_keypair()?;
        // Create credential and add to kernel args
        let pubkey = std::fs::read_to_string(key_pair.public_key_path.as_path())?;
        let credential = crate::sshcred::smbios_cred_for_root_ssh(&pubkey)?;
        qemu_config.add_smbios_credential(credential);
    }
    // Build kernel command line
    let mut kernel_cmdline = vec![
        "rootfstype=virtiofs".to_string(),
        "root=rootfs".to_string(),
        "selinux=0".to_string(),
        "systemd.volatile=overlay".to_string(),
    ];

    if opts.common.console {
        kernel_cmdline.push("console=ttyS0".to_string());
    }

    kernel_cmdline.extend(opts.common.kernel_args.clone());

    // Parse disk files from environment variable
    let mut virtio_blk_devices = Vec::new();
    if let Ok(disk_env) = std::env::var("BOOTC_DISK_FILES") {
        for disk_spec in disk_env.split(',') {
            if let Some((disk_file, disk_name)) = disk_spec.split_once(':') {
                virtio_blk_devices.push(crate::qemu::VirtioBlkDevice {
                    disk_file: disk_file.to_string(),
                    serial: disk_name.to_string(),
                });
            }
        }
    }

    qemu_config
        .set_kernel_cmdline(kernel_cmdline)
        .set_console(opts.common.console);

    if opts.common.ssh_keygen {
        qemu_config.enable_ssh_access(None); // Use default port 2222
        debug!("Enabled SSH port forwarding: host port 2222 -> guest port 22");

        // We need to extract the public key from the SSH credential to inject it via SMBIOS
        // For now, the credential is already being passed via kernel cmdline
        // TODO: Add proper SMBIOS credential injection if needed
    }

    // Set main virtiofs configuration for root filesystem (will be spawned by QEMU)
    qemu_config.set_main_virtiofs(main_virtiofsd_config.clone());

    // Add additional virtiofs configurations (will be spawned by QEMU)
    for (virtiofs_config, tag) in additional_mounts {
        qemu_config.add_virtiofs(virtiofs_config, &tag);
    }

    let exec_pipes = if !opts.common.execute.is_empty() {
        let execute_pipefd: File = qemu_config.add_virtio_serial_pipe("execute")?.into();
        let status_pipefd: File = qemu_config.add_virtio_serial_pipe("executestatus")?.into();
        Some((execute_pipefd, status_pipefd))
    } else {
        None
    };

    // Add virtio-blk devices
    for blk_device in virtio_blk_devices {
        qemu_config.add_virtio_blk_device(blk_device.disk_file, blk_device.serial);
    }

    // Only enable systemd notification debugging if the systemd version supports it
    if systemd_version.has_vmm_notify() {
        let log_path = Utf8Path::new("/run/systemd-guest.txt");
        let logf = File::create(log_path).context("Creating log")?;
        qemu_config.systemd_notify = Some(logf);
        debug!(
            "systemd {} supports vmm.notify_socket, enabling systemd notification debugging",
            systemd_version.0
        );
    } else {
        debug!(
            "systemd {} does not support vmm.notify_socket",
            systemd_version.0
        );
    }

    debug!("Starting QEMU with systemd debugging enabled");

    // Spawn QEMU with all virtiofsd processes handled internally
    let mut qemu = crate::qemu::RunningQemu::spawn(qemu_config).await?;

    // Handle execute command output streaming if needed
    if let Some((exec_pipefd, status_pipefd)) = exec_pipes {
        tracing::debug!("Starting execute output streaming with pipes");
        let output_copier = async move {
            let fd = tokio::fs::File::from(exec_pipefd);
            let mut bufr = tokio::io::BufReader::new(fd);
            let mut stdout = tokio::io::stdout();
            let result = tokio::io::copy(&mut bufr, &mut stdout).await;
            tracing::debug!("Output copy result: {:?}", result);
            result
        };
        let mut status_reader = tokio::io::BufReader::new(tokio::fs::File::from(status_pipefd));
        let mut status = String::new();
        let status_reader = status_reader.read_to_string(&mut status);

        // And wait for all tasks
        let (qemu, output_copier, execstatus) =
            tokio::join!(qemu.wait(), output_copier, status_reader);
        // Do check for errors from reading from the execstatus pipe
        let _ = execstatus.context("Reading execstatus")?;

        // Discard errors from qemu and the output copier
        tracing::debug!("qemu exit status: {qemu:?}");
        tracing::debug!("output copy: {output_copier:?}");

        // Parse exit code from systemd service status
        let exit_code = parse_service_exit_code(&status)?;
        if exit_code != 0 {
            return Err(eyre!(
                "Execute command failed with exit code: {}",
                exit_code
            ));
        }
    } else {
        // Wait for QEMU to complete
        let exit_status = qemu.wait().await?;
        if !exit_status.success() {
            return Err(eyre!("QEMU exited with non-zero status: {}", exit_status));
        }
    }

    debug!("QEMU completed successfully");

    Ok(())
}
