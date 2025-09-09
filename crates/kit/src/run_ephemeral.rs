//! Ephemeral VM execution using hybrid container-VM approach.
//!
//! Runs QEMU inside privileged Podman containers with KVM access,
//! host /usr bind-mount, and virtiofs for container image filesystem.
//! Supports SSH access, command execution, and host directory mounts.

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;

use clap::Parser;
use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use itertools::Itertools;
use rustix::path::Arg;
use tracing::debug;

use crate::{podman, utils};
use std::process::Child;

/// Common container lifecycle options for podman commands.
#[derive(Parser, Debug, Clone, Default)]
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
}

/// Common VM configuration options for hardware, networking, and features.
#[derive(Parser, Debug, Clone, Default)]
pub struct CommonVmOpts {
    #[clap(
        long,
        help = "Memory size (e.g. 2G, 1024M, 512m, or plain number for MB) [default: 2048]"
    )]
    pub memory: Option<String>,

    #[clap(long, help = "Number of vCPUs [default: 2]")]
    pub vcpus: Option<u32>,

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
        conflicts_with = "execute_sh",
        help = "Execute command inside VM via systemd and capture output"
    )]
    pub execute: Option<String>,

    #[clap(
        long,
        conflicts_with = "execute",
        help = "Execute shell script inside VM and capture output"
    )]
    pub execute_sh: Option<String>,

    #[clap(long, short = 'K', help = "Generate SSH keypair and enable SSH access")]
    pub ssh_keygen: bool,

    #[clap(
        long,
        default_value = "root",
        help = "SSH username for key injection and connections"
    )]
    pub ssh_user: String,

    #[clap(
        long,
        conflicts_with = "ssh_keygen",
        help = "Use existing SSH private key file instead of generating new"
    )]
    pub ssh_identity: Option<String>,
}

impl CommonVmOpts {
    /// Parse memory specification to MB (default: 2048)
    pub fn memory_mb(&self) -> color_eyre::Result<u32> {
        if let Some(ref mem_str) = self.memory {
            crate::utils::parse_memory_to_mb(mem_str)
        } else {
            Ok(2048)
        }
    }

    /// Get vCPU count (default: 2)
    pub fn vcpus(&self) -> u32 {
        self.vcpus.unwrap_or(2)
    }

    /// Get network config (default: "none")
    pub fn net_string(&self) -> String {
        self.net.clone().unwrap_or_else(|| "none".to_string())
    }
}

/// Ephemeral VM options: container-style flags, host bind mounts, systemd injection.
#[derive(Parser, Debug)]
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

/// Internal VM options used by container entrypoint (not for direct use).
#[derive(Parser, Debug)]
pub struct RunEphemeralImplOpts {
    #[clap(long, help = "Memory in MiB (processed from user input)")]
    pub memory: u32,

    #[clap(long, help = "Number of vCPUs (processed from user input)")]
    pub vcpus: u32,

    #[clap(long, help = "Complete kernel command line arguments")]
    pub extra_args: Option<String>,

    #[clap(long, help = "Enable console output to terminal")]
    pub console: bool,
}

/// Launch privileged container with QEMU+KVM for ephemeral VM.
/// Returns (container_status, command_exit_code) - command exit takes precedence.
pub fn run_qemu_in_container(
    opts: &RunEphemeralOpts,
    entrypoint_cmd: Option<&str>,
) -> Result<(std::process::ExitStatus, Option<i32>)> {
    debug!("Running QEMU inside hybrid container for {}", opts.image);

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

    let mut extra_args = opts.common.kernel_args.iter().map(|s| s.as_str()).join(" ");

    // Handle SSH key generation and credential injection
    let ssh_key_path = if opts.common.ssh_keygen || opts.common.ssh_identity.is_some() {
        if let Some(ref identity_path) = opts.common.ssh_identity {
            // Use existing SSH key
            Some(identity_path.clone())
        } else {
            // Generate new SSH key
            let vm_id = crate::ssh::generate_vm_id();
            let cache_dir = crate::ssh::get_vm_cache_dir(&vm_id)?;
            let key_pair = crate::ssh::generate_ssh_keypair(&cache_dir, "ssh_key")?;

            // Create credential and add to kernel args
            let credential = crate::sshcred::karg_for_root_ssh(&key_pair.public_key_content)?;
            if !extra_args.is_empty() {
                extra_args.push(' ');
            }
            extra_args.push_str(&credential);

            debug!("Generated SSH key and added credential to kernel args");
            Some(key_pair.private_key_path.to_string_lossy().to_string())
        }
    } else {
        None
    };

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
            storage_path.display()
        );
        host_mounts.push((
            storage_path.display().to_string(),
            "hoststorage".to_string(),
            true,
        )); // true = read-only
    }

    // Parse writable bind mounts
    for mount_spec in &opts.bind_mounts {
        let (host_path, mount_name) = if let Some((path, name)) = mount_spec.split_once(':') {
            (path.to_string(), name.to_string())
        } else {
            let path = mount_spec.clone();
            let name = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("mount"))
                .to_string_lossy()
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
            let name = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("mount"))
                .to_string_lossy()
                .to_string();
            (path, name)
        };
        host_mounts.push((host_path, mount_name, true)); // true = read-only
    }

    // Run the container with the setup script
    let mut cmd = Command::new("podman");
    cmd.arg("run");
    cmd.arg("--label=bootc.kit=1");
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
        "--device=/dev/vsock",
        "-v",
        "/usr:/run/hostusr:ro", // Bind mount host /usr as read-only
        "-v",
        &format!("{entrypoint_path}:/run/entrypoint"),
        "-v",
        &format!("{self_exe}:/run/selfexe:ro"),
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

    // Mount SSH key if available
    if let Some(ref key_path) = ssh_key_path {
        cmd.args(["-v", &format!("{}:/tmp/ssh:ro", key_path)]);
        debug!("Mounted SSH key at /tmp/ssh");
    }

    // Set debug mode environment variable if requested
    if opts.common.debug {
        cmd.args(["-e", "DEBUG_MODE=true"]);
        debug!("Debug mode enabled - will drop into shell instead of running QEMU");
    }

    // Pass configuration as JSON via BCK_CONFIG environment variable
    let config = crate::container_entrypoint::ContainerConfig {
        memory_mb: opts.common.memory_mb()?,
        vcpus: opts.common.vcpus(),
        console: opts.common.console,
        extra_args: if extra_args.is_empty() {
            None
        } else {
            Some(extra_args.clone())
        },
    };

    let config_json = serde_json::to_string(&config)?;
    cmd.args(["-e", &format!("BCK_CONFIG={}", config_json)]);

    // Pass through BCK_RUN_FROM_INSTALL_CONFIG if it exists
    if let Ok(run_from_install_config) = std::env::var("BCK_RUN_FROM_INSTALL_CONFIG") {
        cmd.args([
            "-e",
            &format!("BCK_RUN_FROM_INSTALL_CONFIG={}", run_from_install_config),
        ]);
        debug!("Passing BCK_RUN_FROM_INSTALL_CONFIG to container");
    }

    // Keep old env vars for backward compatibility (will be removed later)
    cmd.args([
        "-e",
        &format!("BOOTC_MEMORY={}", opts.common.memory_mb()?),
        "-e",
        &format!("BOOTC_VCPUS={}", opts.common.vcpus()),
    ]);

    if !extra_args.is_empty() {
        cmd.args(["-e", &format!("BOOTC_EXTRA_ARGS={}", extra_args)]);
    }

    if opts.common.console {
        cmd.args(["-e", "BOOTC_CONSOLE=1"]);
    }

    // Handle --execute or --execute-sh option
    let mut all_serial_devices = opts.common.virtio_serial_out.clone();
    let execute = opts
        .common
        .execute_sh
        .as_deref()
        .map(|v| Cow::Owned(format!("/bin/sh -c '{}'", v.replace("'", "'\\''"))))
        .or(opts.common.execute.as_deref().map(Cow::Borrowed));
    let (execute_output_file, execute_status_file) = if let Some(execute_cmd) = execute {
        // Create a temp directory that will be mounted into the container
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temporary directory for execute output")?;
        let output_file = temp_dir.path().join("execute-output.txt");
        let status_file = temp_dir.path().join("execute-status.txt");

        debug!(
            "Created temporary directory for execute output at: {:?}",
            temp_dir.path()
        );

        // Create the output and status files so they exist for mounting
        std::fs::File::create(&output_file)
            .with_context(|| format!("Failed to create output file: {}", output_file.display()))?;
        std::fs::File::create(&status_file)
            .with_context(|| format!("Failed to create status file: {}", status_file.display()))?;

        debug!(
            "Created execute output files: {} and {}",
            output_file.display(),
            status_file.display()
        );

        // Mount the temp directory into the container
        cmd.args([
            "-v",
            &format!("{}:/run/execute-output", temp_dir.path().display()),
        ]);

        // Add virtio-serial devices for execute output and status (using the mounted paths)
        all_serial_devices.push(format!("execute:/run/execute-output/execute-output.txt"));
        all_serial_devices.push(format!(
            "executestatus:/run/execute-output/execute-status.txt"
        ));

        cmd.args(["-e", &format!("BOOTC_EXECUTE_CMD={}", execute_cmd)]);
        debug!("Added execute command: {}", execute_cmd);

        let output_path = output_file.clone();
        let status_path = status_file.clone();
        // Keep the temp dir alive by leaking it (we'll clean up on exit)
        let temp_path = temp_dir.into_path();
        debug!(
            "Temp directory will be preserved at: {}",
            temp_path.display()
        );
        (Some(output_path), Some(status_path))
    } else {
        (None, None)
    };

    // Pass virtio-serial devices as environment variable
    if !all_serial_devices.is_empty() {
        let serial_devices = all_serial_devices.join(",");
        cmd.args(["-e", &format!("BOOTC_VIRTIO_SERIAL={}", serial_devices)]);
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

    cmd.args([&opts.image, "/run/entrypoint"]);
    cmd.args(entrypoint_cmd);

    // Log the full command line if requested
    if opts.log_cmdline {
        let args: Vec<String> = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        debug!("Executing: podman {}", args.join(" "));
    }

    // If we have execute output, spawn the container process and stream output concurrently
    // TODO: This streaming logic should live inside the inner implementation, not the outer
    // logic. In fact we should just use Command().exec to truly replace our process with
    // podman after this and not do anything else.
    let status = if let Some(ref output_file) = execute_output_file {
        use std::fs::File;
        use std::io::{BufRead, BufReader, Seek, SeekFrom};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        debug!("Starting container with real-time output streaming");
        let mut child = cmd.spawn().context("Failed to spawn QEMU container")?;

        // Clone the path for the thread
        let output_file_clone = output_file.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();

        let output_thread = thread::spawn(move || {
            let mut file_position = 0u64;
            let mut last_size = 0u64;
            let mut creation_timeout = 0;
            const MAX_CREATION_TIMEOUT: u32 = 100; // 10 seconds

            // Wait for the file to be created with better timeout handling
            while !output_file_clone.exists() && !finished_clone.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
                creation_timeout += 1;
                if creation_timeout >= MAX_CREATION_TIMEOUT {
                    debug!(
                        "Output file creation timeout after {}ms",
                        MAX_CREATION_TIMEOUT * 100
                    );
                    return;
                }
            }

            if !output_file_clone.exists() {
                debug!("Process finished before output file was created");
                return; // Exit early if process finished before file was created
            }

            debug!("Output file created, starting to read");

            loop {
                if let Ok(metadata) = std::fs::metadata(&output_file_clone) {
                    let current_size = metadata.len();
                    if current_size > last_size {
                        if let Ok(mut file) = File::open(&output_file_clone) {
                            if let Ok(new_position) = file.seek(SeekFrom::Start(file_position)) {
                                if new_position != file_position {
                                    debug!(
                                        "File seek position mismatch: expected {}, got {}",
                                        file_position, new_position
                                    );
                                }

                                let reader = BufReader::new(file);
                                let mut bytes_read = 0u64;

                                for line in reader.lines() {
                                    if let Ok(line) = line {
                                        println!("{}", line);
                                        std::io::Write::flush(&mut std::io::stdout()).ok();
                                        // More accurate byte counting: line length + newline character
                                        bytes_read += line.as_bytes().len() as u64 + 1;
                                    }
                                }

                                // Update position based on actual bytes read
                                file_position += bytes_read;
                            }
                            last_size = current_size;
                        }
                    }
                }

                // Check if we should exit
                if finished_clone.load(Ordering::Relaxed) {
                    // Process finished, read any remaining output with improved handling
                    debug!(
                        "Process finished, reading final output from position {}",
                        file_position
                    );
                    if let Ok(mut file) = File::open(&output_file_clone) {
                        if let Ok(_) = file.seek(SeekFrom::Start(file_position)) {
                            let reader = BufReader::new(file);

                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    println!("{}", line);
                                    std::io::Write::flush(&mut std::io::stdout()).ok();
                                }
                            }
                        }
                    }
                    break;
                }

                thread::sleep(Duration::from_millis(100));
            }

            debug!("Output streaming thread finished");
        });

        // Wait for the container to complete
        let status = child.wait().context("Failed to wait for QEMU container")?;

        // Signal the output thread to finish
        finished.store(true, Ordering::Relaxed);

        // Wait for the output thread to finish
        let _ = output_thread.join();

        status
    } else {
        // No execute output, run normally
        cmd.status().context("Failed to run QEMU in container")?
    };

    // Check for execute status and return appropriate exit code
    let execute_exit_code = if let Some(status_file) = execute_status_file {
        debug!("Checking for execute status file: {:?}", status_file);
        if status_file.exists() {
            debug!("Status file exists, reading content");
            let r = std::fs::read_to_string(&status_file)?;
            let lines = r.lines();
            let mut code = None;
            for line in lines {
                let Some(codeval) = line.strip_prefix("ExecMainStatus=") else {
                    continue;
                };
                let codeval: i32 = codeval.parse().context("Parsing ExecMainStatus")?;
                code = Some(codeval);
                break;
            }
            if code.is_none() {
                tracing::warn!("Failed to find ExecMainStatus");
            }
            code
        } else {
            tracing::warn!("Missing status file");
            None
        }
    } else {
        None
    };

    Ok((status, execute_exit_code))
}

/// Main entry point for ephemeral VM execution.
/// Handles exit codes: command execution takes precedence, accepts VM exit code 1 for poweroff.target.
pub fn run(opts: RunEphemeralOpts) -> Result<()> {
    // Run QEMU inside the container with the hybrid rootfs approach
    let (status, execute_exit_code) = run_qemu_in_container(&opts, None)?;

    // If we have an execute command, prioritize its exit code over QEMU's exit code
    if let Some(exit_code) = execute_exit_code {
        if exit_code != 0 {
            return Err(eyre!(
                "Execute command failed with exit code: {}",
                exit_code
            ));
        }
    }

    // QEMU may exit with non-zero status when VM powers off
    // For testing with poweroff.target, we accept exit code 1
    if !status.success() {
        if let Some(code) = status.code() {
            let kargs_str = opts.common.kernel_args.join(" ");
            if code == 1 && kargs_str.contains("poweroff.target") {
                debug!("QEMU exited with code 1 (expected for poweroff.target)");
            } else {
                return Err(eyre!("QEMU exited with non-zero status: {}", code));
            }
        } else {
            return Err(eyre!("QEMU terminated by signal"));
        }
    }

    debug!("VM terminated successfully");
    Ok(())
}

/// Process --mount-disk-file specs: parse file:name format, create sparse files if needed (2x image size),
/// validate only regular files, convert to absolute paths.
pub(crate) fn process_disk_files(
    disk_specs: &[String],
    image: &str,
) -> Result<Vec<(String, String)>> {
    use std::fs::File;
    use std::path::Path;

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

        let disk_path = Path::new(&disk_file);

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
            disk_file
        } else {
            std::fs::canonicalize(&disk_path)
                .with_context(|| format!("Failed to canonicalize disk file path: {}", disk_file))?
                .to_string_lossy()
                .to_string()
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

    let source_units = "/run/systemd-units/system";
    let target_units = "/run/source-image/etc/systemd/system";

    if !std::path::Path::new(source_units).exists() {
        debug!("No system/ directory found in systemd-units, skipping unit injection");
        return Ok(());
    }

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

    if std::path::Path::new(source_wants).exists() {
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

/// RAII guard for automatic virtiofsd process cleanup on drop.
struct VirtiofsdCleanupGuard {
    processes: Vec<Child>,
}

impl VirtiofsdCleanupGuard {
    fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    /// Add virtiofsd process for cleanup tracking
    fn add(&mut self, process: Child) {
        self.processes.push(process);
    }

    /// Kill all tracked processes (called automatically on drop)
    fn cleanup_all(&mut self) {
        for process in &mut self.processes {
            if let Err(e) = process.kill() {
                debug!("Failed to kill virtiofsd process: {}", e);
            }
        }
        self.processes.clear();
    }
}

impl Drop for VirtiofsdCleanupGuard {
    fn drop(&mut self) {
        debug!("Cleaning up {} virtiofsd processes", self.processes.len());
        self.cleanup_all();
    }
}

/// VM execution inside container: extracts kernel/initramfs, starts virtiofsd processes,
/// generates systemd mount units, sets up command execution, launches QEMU.
/// DEBUG_MODE=true drops to shell instead of QEMU.
pub(crate) fn run_impl(opts: RunEphemeralImplOpts) -> Result<()> {
    use crate::qemu;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    debug!("Running QEMU implementation inside container");

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

    // Process host mounts and prepare virtiofsd instances for each
    let mut cleanup_guard = VirtiofsdCleanupGuard::new();
    let mut additional_mounts = Vec::new();

    debug!(
        "Checking for host mounts directory: /run/host-mounts exists = {}",
        std::path::Path::new("/run/host-mounts").exists()
    );
    debug!(
        "Checking for systemd units directory: /run/systemd-units exists = {}",
        std::path::Path::new("/run/systemd-units").exists()
    );

    if std::path::Path::new("/run/host-mounts").exists() {
        // Check if systemd units directory has pre-provided units (read-only from host)
        let has_provided_units = std::path::Path::new("/run/systemd-units/system").exists()
            && fs::read_dir("/run/systemd-units/system")
                .map(|entries| entries.count() > 0)
                .unwrap_or(false);

        if has_provided_units {
            debug!("Systemd units pre-provided - skipping dynamic mount unit creation");
        } else {
            // Use the existing systemd units directory if provided, otherwise create one in /run
            let service_dir = if std::path::Path::new("/run/systemd-units/system").exists() {
                "/run/systemd-units/system".to_string()
            } else {
                let dir = "/run/systemd-units/system";
                fs::create_dir_all(dir).context("Failed to create systemd units directory")?;
                dir.to_string()
            };
            debug!("Using systemd units directory: {}", service_dir);

            for entry in fs::read_dir("/run/host-mounts")? {
                let entry = entry?;
                let mount_name = entry.file_name();
                let mount_name_str = mount_name.to_string_lossy();
                let source_path = entry.path();
                let mount_path = format!("/run/host-mounts/{}", mount_name_str);

                // Check if this directory is mounted as read-only
                let is_readonly = Command::new("findmnt")
                    .args(["-n", "-o", "OPTIONS", &mount_path])
                    .output()
                    .map(|output| {
                        let options = String::from_utf8_lossy(&output.stdout);
                        options.contains("ro")
                    })
                    .unwrap_or(false);

                let mode = if is_readonly { "ro" } else { "rw" };
                debug!(
                    "Setting up virtiofs mount for {} ({})",
                    mount_name_str, mode
                );

                // Create virtiofs socket path and tag
                let socket_path = format!("/run/inner-shared/virtiofs-{}.sock", mount_name_str);
                let tag = format!("mount_{}", mount_name_str);

                // Spawn virtiofsd for this mount
                let virtiofsd_config = qemu::VirtiofsConfig {
                    socket_path: socket_path.clone(),
                    shared_dir: source_path.to_string_lossy().to_string(),
                    cache_mode: "always".to_string(),
                    sandbox: "none".to_string(),
                    debug: debug_mode,
                };
                let virtiofsd_instance = qemu::spawn_virtiofsd(&virtiofsd_config)?;
                cleanup_guard.add(virtiofsd_instance);

                // Wait for this virtiofsd socket to be ready
                qemu::wait_for_virtiofsd_socket(&socket_path, Duration::from_secs(10))?;

                // Add to QEMU mounts
                additional_mounts.push(crate::qemu::VirtiofsMount {
                    socket_path: socket_path.clone(),
                    tag: tag.clone(),
                });

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
Wants=local-fs.target

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

                let mount_unit_path = format!("{}/{}", service_dir, mount_unit_name);
                fs::write(&mount_unit_path, mount_unit_content).with_context(|| {
                    format!("Failed to write mount unit to {}", mount_unit_path)
                })?;

                // Enable the mount unit by creating symlink in local-fs.target.wants/
                let wants_dir = format!("{}/local-fs.target.wants", service_dir);
                fs::create_dir_all(&wants_dir).ok();
                let wants_link = format!("{}/{}", wants_dir, mount_unit_name);
                let relative_target = format!("../{}", mount_unit_name);
                std::os::unix::fs::symlink(&relative_target, &wants_link).ok();

                // Create mount point directory in the image
                let image_mount_point = format!("/run/source-image{}", mount_point);
                fs::create_dir_all(&image_mount_point).ok();

                debug!(
                    "Generated mount unit: {} (enabled in local-fs.target)",
                    mount_unit_name
                );
            }
        } // end else block for has_provided_units check
    }

    // Handle execute command or script by creating systemd unit directly
    let execute_cmd = std::env::var("BOOTC_EXECUTE_CMD").ok();

    if let Some(exec_cmd) = execute_cmd {
        // Create systemd units directory if it doesn't exist
        if !std::path::Path::new("/run/systemd-units/system").exists() {
            fs::create_dir_all("/run/systemd-units/system")?;
            fs::create_dir_all("/run/systemd-units/system/default.target.wants")?;
        }

        let service_content = format!(
            r#"[Unit]
Description=Execute Script Service
Requires=dev-virtio\\x2dports-execute.device

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart={exec_cmd}
StandardOutput=file:/dev/virtio-ports/execute
StandardError=inherit
"#
        );
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

        let service_path = "/run/systemd-units/system/bootc-execute.service";
        fs::write(service_path, service_content)?;
        let service_path = "/run/systemd-units/system/bootc-execute-finish.service";
        fs::write(service_path, service_finish)?;

        // Create wants directory and symlink to enable the service
        let wants_dir = "/run/systemd-units/system/default.target.wants";
        std::fs::create_dir_all(wants_dir)?;

        for svc in ["bootc-execute.service", "bootc-execute-finish.service"] {
            let wants_link = format!("/run/systemd-units/system/default.target.wants/{svc}");
            debug!("Creating execute service symlink: {}", &wants_link);
            std::os::unix::fs::symlink(format!("../{svc}"), wants_link)?;
        }
    }

    // Copy systemd units if provided (after mount units have been generated)
    // Also inject if we created mount units that need to be copied
    if std::path::Path::new("/run/systemd-units").exists() {
        inject_systemd_units()?;
    }

    // Start virtiofsd in background using the source image directly
    // If we have host mounts, we'll need QEMU to mount them separately
    let mut virtiofsd_config = qemu::VirtiofsConfig::default();
    virtiofsd_config.debug = debug_mode;
    let virtiofsd = qemu::spawn_virtiofsd(&virtiofsd_config)?;
    cleanup_guard.add(virtiofsd);

    // Wait for socket to be created with proper checking
    qemu::wait_for_virtiofsd_socket(&virtiofsd_config.socket_path, Duration::from_secs(10))?;

    if debug_mode {
        debug!("=== DEBUG MODE: Dropping into bash shell ===");
        debug!("Environment setup complete. You can:");
        debug!("- Inspect /run/tmproot (the hybrid rootfs)");
        debug!("- Check virtiofsd socket at /run/inner-shared/virtiofs.sock");
        debug!("- Exit with 'exit' to terminate");

        let status = Command::new("bash")
            .status()
            .context("Failed to start debug shell")?;

        // Cleanup guard will automatically clean up virtiofsd processes on drop

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

        // Parse virtio-serial-out arguments from environment variable
        let mut virtio_serial_devices = Vec::new();
        if let Ok(serial_env) = std::env::var("BOOTC_VIRTIO_SERIAL") {
            for serial_spec in serial_env.split(',') {
                if let Some((name, output_file)) = serial_spec.split_once(':') {
                    virtio_serial_devices.push(crate::qemu::VirtioSerialOut {
                        name: name.to_string(),
                        output_file: output_file.to_string(),
                    });
                }
            }
        }

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

        // Configure and start QEMU
        let mut qemu_config = crate::qemu::QemuConfig::new_direct_boot(
            opts.memory,
            opts.vcpus,
            "/run/qemu/kernel".to_string(),
            "/run/qemu/initramfs".to_string(),
            virtiofsd_config.socket_path.clone(),
        );

        qemu_config
            .set_kernel_cmdline(kernel_cmdline)
            .set_console(opts.console);

        // Enable SSH port forwarding if SSH credentials are present in extra args
        let has_ssh_credentials = if let Some(ref extra_args) = opts.extra_args {
            debug!("Checking extra_args for SSH credentials: {}", extra_args);
            let has_creds = extra_args.contains("systemd.set_credential_binary=tmpfiles.extra:")
                || extra_args.contains("io.systemd.credential.binary:tmpfiles.extra=");
            debug!("SSH credentials detected: {}", has_creds);
            has_creds
        } else {
            debug!("No extra_args provided");
            false
        };

        if has_ssh_credentials {
            qemu_config.enable_ssh_access(None); // Use default port 2222
            debug!("Enabled SSH port forwarding: host port 2222 -> guest port 22");

            // We need to extract the public key from the SSH credential to inject it via SMBIOS
            // For now, the credential is already being passed via kernel cmdline
            // TODO: Add proper SMBIOS credential injection if needed
        }

        // Add additional mounts
        for mount in additional_mounts {
            qemu_config.add_virtiofs_mount(mount.socket_path, mount.tag);
        }

        // Add virtio-serial devices
        for serial_device in virtio_serial_devices {
            qemu_config.add_virtio_serial_out(serial_device.name, serial_device.output_file);
        }

        // Add virtio-blk devices
        for blk_device in virtio_blk_devices {
            qemu_config.add_virtio_blk_device(blk_device.disk_file, blk_device.serial);
        }

        // Enable AF_VSOCK systemd debugging for better boot analysis
        debug!("Enabling AF_VSOCK systemd debugging for ephemeral VM");
        if let Err(e) = qemu_config.enable_vsock() {
            // Don't fail the entire operation if vsock debugging fails
            debug!(
                "Failed to enable vsock debugging (continuing anyway): {}",
                e
            );
        }

        let log_path = Path::new("/run/systemd-guest.txt");
        let logf = File::create(log_path).context("Creating log")?;
        qemu_config.systemd_notify = Some(logf);

        debug!("Starting QEMU with systemd debugging enabled");
        let mut qemu = crate::qemu::RunningQemu::spawn(qemu_config)?;

        qemu.wait()?;
        debug!("QEMU completed successfully");
    } // Close the else block

    Ok(())
}
