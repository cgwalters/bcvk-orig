//! QEMU virtualization integration and VM management.
//!
//! Supports direct kernel boot and disk image boot with VirtIO devices,
//! automatic process cleanup, and SMBIOS credential injection.

use std::fs::File;
use std::io::ErrorKind;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use cap_std_ext::cmdext::CapStdExtCommandExt;
use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use libc::{VMADDR_CID_ANY, VMADDR_PORT_ANY};
use nix::sys::socket::{accept, bind, getsockname, socket, AddressFamily, SockFlag, SockType};
use tracing::{debug, warn};
use vsock::VsockAddr;

/// VirtIO-FS mount point configuration.
#[derive(Debug, Clone)]
pub struct VirtiofsMount {
    /// Unix socket path for virtiofsd communication
    pub socket_path: String,
    /// Mount tag used by guest to identify this mount
    pub tag: String,
}

/// VirtIO-Serial device for guest-to-host communication.
/// Appears as /dev/virtio-ports/{name} in guest.
#[derive(Debug)]
pub struct VirtioSerialOut {
    /// Device name (becomes /dev/virtio-ports/{name})
    pub name: String,
    /// Host file path for output
    pub output_file: String,
}

/// VirtIO-Block storage device configuration.
/// Appears as /dev/disk/by-id/virtio-{serial} in guest.
#[derive(Debug)]
pub struct VirtioBlkDevice {
    /// Host disk image file path
    pub disk_file: String,
    /// Device serial for guest identification
    pub serial: String,
}

/// VM display and console configuration.
#[derive(Debug, Clone)]
pub enum DisplayMode {
    /// Headless mode (-nographic)
    None,
    /// Console to stdio (-serial stdio -display none)
    Console,
}

impl Default for DisplayMode {
    fn default() -> Self {
        DisplayMode::None
    }
}

/// VM network configuration.
#[derive(Debug, Clone)]
pub enum NetworkMode {
    /// User-mode networking with NAT and port forwarding
    User {
        /// Port forwarding rules: "tcp::2222-:22" format
        hostfwd: Vec<String>,
    },
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::User { hostfwd: vec![] }
    }
}

/// Resource limits for QEMU processes.
/// Note: Applied externally via taskset/ionice/nice, not QEMU args.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// CPU affinity bitmask ("0xF" for cores 0-3)
    pub cpu_affinity: Option<String>,
    /// I/O priority (0=highest, 7=lowest)
    pub io_priority: Option<u8>,
    /// Nice level (-20=highest, 19=lowest)
    pub nice_level: Option<i8>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_affinity: None,
            io_priority: None,
            nice_level: None,
        }
    }
}

/// VM boot configuration: direct kernel boot or disk image boot.
#[derive(Debug)]
pub enum BootMode {
    /// Direct kernel boot (fast, testing-focused)
    DirectBoot {
        kernel_path: String,
        initramfs_path: String,
        kernel_cmdline: Vec<String>,
        /// VirtIO-FS socket for root filesystem
        virtiofs_socket: String,
    },
    #[allow(dead_code)]
    DiskBoot {
        primary_disk: String,
        /// Use UEFI instead of BIOS
        uefi: bool,
    },
}

/// Complete QEMU VM configuration with builder pattern.
#[derive(Debug)]
pub struct QemuConfig {
    /// RAM in megabytes (128MB-1TB)
    pub memory_mb: u32,
    /// Number of vCPUs (1-256)
    pub vcpus: u32,
    pub boot_mode: BootMode,
    /// Main VirtioFS configuration for root filesystem (handled separately from additional mounts)
    pub main_virtiofs_config: Option<VirtiofsConfig>,
    /// VirtioFS configurations to spawn as daemons
    pub virtiofs_configs: Vec<VirtiofsConfig>,
    /// Additional VirtIO-FS mounts
    pub additional_mounts: Vec<VirtiofsMount>,
    pub virtio_serial_devices: Vec<VirtioSerialOut>,
    pub virtio_blk_devices: Vec<VirtioBlkDevice>,
    pub display_mode: DisplayMode,
    pub network_mode: NetworkMode,
    pub resource_limits: ResourceLimits,
    /// Deprecated: use display_mode
    pub enable_console: bool,
    /// UEFI firmware path (auto-detected if None)
    pub uefi_firmware_path: Option<String>,
    /// UEFI variables file
    pub uefi_vars_path: Option<String>,
    /// SMBIOS credentials for systemd
    pub smbios_credentials: Vec<String>,
    /// VSOCK is enabled by default
    pub disable_vsock: bool,

    /// Write systemd notifications to this file
    pub systemd_notify: Option<File>,
}

impl QemuConfig {
    /// Create a new config with direct boot (kernel + initramfs)
    pub fn new_direct_boot(
        memory_mb: u32,
        vcpus: u32,
        kernel_path: String,
        initramfs_path: String,
        virtiofs_socket: String,
    ) -> Self {
        Self {
            memory_mb,
            vcpus,
            boot_mode: BootMode::DirectBoot {
                kernel_path,
                initramfs_path,
                kernel_cmdline: vec![],
                virtiofs_socket,
            },
            main_virtiofs_config: None,
            virtiofs_configs: vec![],
            additional_mounts: vec![],
            virtio_serial_devices: vec![],
            virtio_blk_devices: vec![],
            display_mode: DisplayMode::default(),
            network_mode: NetworkMode::default(),
            resource_limits: ResourceLimits::default(),
            enable_console: false,
            uefi_firmware_path: None,
            uefi_vars_path: None,
            smbios_credentials: vec![],
            disable_vsock: false,
            systemd_notify: None,
        }
    }

    /// Create a new config with disk boot
    pub fn new_disk_boot(memory_mb: u32, vcpus: u32, primary_disk: String) -> Self {
        Self {
            memory_mb,
            vcpus,
            boot_mode: BootMode::DiskBoot {
                primary_disk,
                uefi: false,
            },
            main_virtiofs_config: None,
            virtiofs_configs: vec![],
            additional_mounts: vec![],
            virtio_serial_devices: vec![],
            virtio_blk_devices: vec![],
            display_mode: DisplayMode::default(),
            network_mode: NetworkMode::default(),
            resource_limits: ResourceLimits::default(),
            enable_console: false,
            uefi_firmware_path: None,
            uefi_vars_path: None,
            smbios_credentials: vec![],
            disable_vsock: false,
            systemd_notify: None,
        }
    }

    /// Set kernel command line arguments (only for direct boot)
    pub fn set_kernel_cmdline(&mut self, cmdline: Vec<String>) -> &mut Self {
        if let BootMode::DirectBoot { kernel_cmdline, .. } = &mut self.boot_mode {
            *kernel_cmdline = cmdline;
        }
        self
    }

    /// Enable UEFI boot (only for disk boot)
    #[allow(dead_code)]
    pub fn set_uefi_boot(&mut self, uefi: bool) -> &mut Self {
        if let BootMode::DiskBoot {
            uefi: uefi_flag, ..
        } = &mut self.boot_mode
        {
            *uefi_flag = uefi;
        }
        self
    }

    /// Enable console output
    pub fn set_console(&mut self, enable: bool) -> &mut Self {
        self.enable_console = enable;
        if enable {
            self.display_mode = DisplayMode::Console;
        }
        self
    }

    /// Validate configuration before VM creation
    pub fn validate(&self) -> Result<()> {
        // Memory validation
        if self.memory_mb < 128 {
            return Err(eyre!(
                "Memory too low: {}MB (minimum 128MB)",
                self.memory_mb
            ));
        }
        if self.memory_mb > 1024 * 1024 {
            return Err(eyre!("Memory too high: {}MB (maximum 1TB)", self.memory_mb));
        }

        // CPU validation
        if self.vcpus == 0 {
            return Err(eyre!("vCPU count must be at least 1"));
        }
        if self.vcpus > 256 {
            return Err(eyre!("vCPU count too high: {} (maximum 256)", self.vcpus));
        }

        // Boot mode validation
        match &self.boot_mode {
            BootMode::DirectBoot {
                kernel_path,
                initramfs_path,
                virtiofs_socket,
                ..
            } => {
                if !std::path::Path::new(kernel_path).exists() {
                    return Err(eyre!("Kernel file does not exist: {}", kernel_path));
                }
                if !std::path::Path::new(initramfs_path).exists() {
                    return Err(eyre!("Initramfs file does not exist: {}", initramfs_path));
                }
                let socket_dir = std::path::Path::new(virtiofs_socket)
                    .parent()
                    .ok_or_else(|| eyre!("Invalid virtiofs socket path: {}", virtiofs_socket))?;
                if !socket_dir.exists() {
                    return Err(eyre!(
                        "Virtiofs socket directory does not exist: {}",
                        socket_dir.display()
                    ));
                }
            }
            BootMode::DiskBoot { primary_disk, uefi } => {
                if !std::path::Path::new(primary_disk).exists() {
                    return Err(eyre!("Primary disk image does not exist: {}", primary_disk));
                }
                if *uefi {
                    if let Some(ref firmware_path) = self.uefi_firmware_path {
                        if !std::path::Path::new(firmware_path).exists() {
                            return Err(eyre!(
                                "UEFI firmware file does not exist: {}",
                                firmware_path
                            ));
                        }
                    } else {
                        return Err(eyre!("UEFI boot enabled but no firmware path specified"));
                    }
                }
            }
        }

        // Validate virtio block devices
        for blk_device in &self.virtio_blk_devices {
            if !std::path::Path::new(&blk_device.disk_file).exists() {
                return Err(eyre!(
                    "Virtio block device file does not exist: {}",
                    blk_device.disk_file
                ));
            }
            if blk_device.serial.is_empty() {
                return Err(eyre!("Virtio block device serial cannot be empty"));
            }
        }

        // Validate virtio serial devices
        for serial_device in &self.virtio_serial_devices {
            if serial_device.name.is_empty() {
                return Err(eyre!("Virtio serial device name cannot be empty"));
            }
            let output_dir = std::path::Path::new(&serial_device.output_file)
                .parent()
                .ok_or_else(|| {
                    eyre!(
                        "Invalid virtio serial output file path: {}",
                        serial_device.output_file
                    )
                })?;
            if !output_dir.exists() {
                return Err(eyre!(
                    "Virtio serial output directory does not exist: {}",
                    output_dir.display()
                ));
            }
        }

        // Validate virtiofs mounts
        for mount in &self.additional_mounts {
            if mount.tag.is_empty() {
                return Err(eyre!("Virtiofs mount tag cannot be empty"));
            }
            let socket_dir = std::path::Path::new(&mount.socket_path)
                .parent()
                .ok_or_else(|| eyre!("Invalid virtiofs socket path: {}", mount.socket_path))?;
            if !socket_dir.exists() {
                return Err(eyre!(
                    "Virtiofs socket directory does not exist: {}",
                    socket_dir.display()
                ));
            }
        }

        Ok(())
    }

    /// Add a virtio-blk device
    pub fn add_virtio_blk_device(&mut self, disk_file: String, serial: String) -> &mut Self {
        self.virtio_blk_devices
            .push(VirtioBlkDevice { disk_file, serial });
        self
    }

    /// Set the main virtiofs configuration for the root filesystem
    pub fn set_main_virtiofs(&mut self, config: VirtiofsConfig) -> &mut Self {
        self.main_virtiofs_config = Some(config);
        self
    }

    /// Add a virtiofs configuration that will be spawned as a daemon
    pub fn add_virtiofs(&mut self, config: VirtiofsConfig) -> &mut Self {
        // Also add a corresponding mount so QEMU knows about it
        self.additional_mounts.push(VirtiofsMount {
            socket_path: config.socket_path.clone(),
            tag: format!("virtiofs-{}", self.virtiofs_configs.len()),
        });
        self.virtiofs_configs.push(config);
        self
    }

    /// Add a virtiofs mount (for pre-spawned daemons)
    pub fn add_virtiofs_mount(&mut self, socket_path: String, tag: String) -> &mut Self {
        self.additional_mounts
            .push(VirtiofsMount { socket_path, tag });
        self
    }

    /// Add a virtio-serial output device
    pub fn add_virtio_serial_out(&mut self, name: String, output_file: String) -> &mut Self {
        self.virtio_serial_devices
            .push(VirtioSerialOut { name, output_file });
        self
    }

    /// Add SMBIOS credential for systemd credential passing
    pub fn add_smbios_credential(&mut self, credential: String) -> &mut Self {
        self.smbios_credentials.push(credential);
        self
    }

    /// Enable SSH access by configuring port forwarding
    pub fn enable_ssh_access(&mut self, host_port: Option<u16>) -> &mut Self {
        let port = host_port.unwrap_or(2222); // Default to port 2222 on host
        let hostfwd = format!("tcp::{}-:22", port); // Forward host port to guest port 22
        self.network_mode = NetworkMode::User {
            hostfwd: vec![hostfwd],
        };
        self
    }
}

/// Allocate a unique VSOCK CID
fn allocate_vsock_cid() -> Result<(OwnedFd, u32)> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    let vhost_fd = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/vhost-vsock")
        .context("Failed to open /dev/vhost-vsock for CID allocation")?;

    for candidate_cid in 3..10001u32 {
        // Test if this CID is available
        // VHOST_VSOCK_SET_GUEST_CID = _IOW(VHOST_VIRTIO, 0x60, __u64)
        const VHOST_VSOCK_SET_GUEST_CID: libc::c_ulong = 0x4008af60;

        let cid = candidate_cid as u64;
        let result = unsafe {
            match libc::ioctl(
                vhost_fd.as_raw_fd(),
                VHOST_VSOCK_SET_GUEST_CID,
                &cid as *const u64,
            ) {
                0 => Ok(()),
                _ => Err(std::io::Error::last_os_error()),
            }
        };
        match result {
            Ok(()) => {
                // Success! This CID is available
                debug!("Successfully allocated VSOCK CID: {}", candidate_cid);
                return Ok((vhost_fd.into(), candidate_cid));
            }
            Err(e) if e.kind() == ErrorKind::AddrInUse => {
                debug!("VSOCK CID {} is in use, trying next", candidate_cid);
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }

    Err(eyre!("Could not find available VSOCK CID (tried 3-10000)"))
}

/// Spawn QEMU VM process with given configuration and optional extra credential.
/// Uses KVM acceleration, memory-backend-memfd for VirtIO-FS compatibility.
fn spawn(
    config: &QemuConfig,
    extra_credentials: &[String],
    vsock: Option<(OwnedFd, u32)>,
) -> Result<Child> {
    // Validate configuration first
    config.validate()?;
    let memory_arg = format!("{}M", config.memory_mb);
    let memory_obj_arg = format!(
        "memory-backend-memfd,id=mem,share=on,size={}M",
        config.memory_mb
    );

    let mut cmd = Command::new("qemu-kvm");
    // SAFETY: This API is safe to call in a forked child.
    unsafe {
        cmd.pre_exec(|| {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))
                .map_err(Into::into)
        });
    }
    cmd.args([
        "-m",
        &memory_arg,
        "-smp",
        &config.vcpus.to_string(),
        "-enable-kvm",
        "-cpu",
        "host",
        "-audio",
        "none",
        "-object",
        &memory_obj_arg,
        "-numa",
        "node,memdev=mem",
    ]);

    // Configure boot mode
    match &config.boot_mode {
        BootMode::DirectBoot {
            kernel_path,
            initramfs_path,
            kernel_cmdline,
            virtiofs_socket,
        } => {
            // Direct kernel boot
            cmd.args(["-kernel", kernel_path, "-initrd", initramfs_path]);

            // Add virtiofs root mount for direct boot
            cmd.args([
                "-chardev",
                &format!("socket,id=char0,path={}", virtiofs_socket),
                "-device",
                "vhost-user-fs-pci,queue-size=1024,chardev=char0,tag=rootfs",
            ]);

            // Add kernel command line
            let append_str = kernel_cmdline.join(" ");
            cmd.args(["-append", &append_str]);
        }
        BootMode::DiskBoot { primary_disk, uefi } => {
            // Configure UEFI firmware if requested
            if *uefi {
                if let Some(ref firmware_path) = config.uefi_firmware_path {
                    // UEFI firmware configuration
                    cmd.args([
                        "-drive",
                        &format!("if=pflash,format=raw,readonly=on,file={}", firmware_path),
                    ]);

                    // UEFI variables (if specified)
                    if let Some(ref vars_path) = config.uefi_vars_path {
                        cmd.args([
                            "-drive",
                            &format!("if=pflash,format=raw,file={}", vars_path),
                        ]);
                    }

                    // Disable default SeaBIOS
                    cmd.args(["-machine", "q35"]);
                    debug!("UEFI boot configured with firmware: {}", firmware_path);
                } else {
                    // Try to auto-detect UEFI firmware paths
                    let common_uefi_paths = [
                        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
                        "/usr/share/OVMF/OVMF_CODE.fd",
                        "/usr/share/ovmf/OVMF.fd",
                    ];

                    let mut found_firmware = None;
                    for path in &common_uefi_paths {
                        if std::path::Path::new(path).exists() {
                            found_firmware = Some(path);
                            break;
                        }
                    }

                    if let Some(firmware_path) = found_firmware {
                        cmd.args([
                            "-drive",
                            &format!("if=pflash,format=raw,readonly=on,file={}", firmware_path),
                        ]);
                        cmd.args(["-machine", "q35"]);
                        debug!(
                            "UEFI boot configured with auto-detected firmware: {}",
                            firmware_path
                        );
                    } else {
                        warn!("UEFI boot requested but no firmware found, falling back to BIOS");
                    }
                }
            }

            // Add primary boot disk
            cmd.args([
                "-drive",
                &format!("file={},format=raw,if=none,id=boot_drive", primary_disk),
                "-device",
                "virtio-blk-pci,drive=boot_drive,serial=boot_disk,bootindex=1",
            ]);
        }
    }

    // Add additional virtiofs mounts
    for (idx, mount) in config.additional_mounts.iter().enumerate() {
        let char_id = format!("char{}", idx + 1);
        cmd.args([
            "-chardev",
            &format!("socket,id={},path={}", char_id, mount.socket_path),
            "-device",
            &format!(
                "vhost-user-fs-pci,queue-size=1024,chardev={},tag={}",
                char_id, mount.tag
            ),
        ]);
    }

    // Add virtio-serial devices
    if !config.virtio_serial_devices.is_empty() {
        // Add the virtio-serial controller
        cmd.args(["-device", "virtio-serial"]);

        for (idx, serial_device) in config.virtio_serial_devices.iter().enumerate() {
            let char_id = format!("serial_char{}", idx);
            cmd.args([
                "-chardev",
                &format!("file,id={},path={}", char_id, serial_device.output_file),
                "-device",
                &format!(
                    "virtserialport,chardev={},name={}",
                    char_id, serial_device.name
                ),
            ]);
        }
    }

    // Add virtio-blk block devices
    for (idx, blk_device) in config.virtio_blk_devices.iter().enumerate() {
        let drive_id = format!("drive{}", idx);
        cmd.args([
            "-drive",
            &format!(
                "file={},format=raw,if=none,id={}",
                blk_device.disk_file, drive_id
            ),
            "-device",
            &format!(
                "virtio-blk-pci,drive={},serial={}",
                drive_id, blk_device.serial
            ),
        ]);
    }

    // Configure network (only User mode supported now)
    match &config.network_mode {
        NetworkMode::User { hostfwd } => {
            if hostfwd.is_empty() {
                cmd.args([
                    "-netdev",
                    "user,id=net0",
                    "-device",
                    "virtio-net-pci,netdev=net0",
                ]);
            } else {
                let hostfwd_arg = format!("user,id=net0,hostfwd={}", hostfwd.join(",hostfwd="));
                cmd.args([
                    "-netdev",
                    &hostfwd_arg,
                    "-device",
                    "virtio-net-pci,netdev=net0",
                ]);
            }
        }
    }

    // Configure display and console (only None and Console modes supported now)
    match &config.display_mode {
        DisplayMode::None => {
            cmd.args(["-nographic"]);
        }
        DisplayMode::Console => {
            cmd.args(["-serial", "stdio", "-display", "none"]);
        }
    }

    // Apply resource limits
    if let Some(affinity) = &config.resource_limits.cpu_affinity {
        // Note: CPU affinity is typically set via taskset or systemd, not QEMU args
        debug!("CPU affinity requested: {} (apply externally)", affinity);
    }

    if let Some(io_priority) = config.resource_limits.io_priority {
        // Note: I/O priority is typically set via ionice, not QEMU args
        debug!("I/O priority requested: {} (apply externally)", io_priority);
    }

    if let Some(nice_level) = config.resource_limits.nice_level {
        // Note: Nice level is typically set via nice command, not QEMU args
        debug!("Nice level requested: {} (apply externally)", nice_level);
    }

    // Add AF_VSOCK device if enabled
    if let Some((vhostfd, guest_cid)) = vsock {
        debug!("Adding AF_VSOCK device with guest CID: {}", guest_cid);
        cmd.take_fd_n(Arc::new(vhostfd), 42);
        cmd.args([
            "-device",
            &format!("vhost-vsock-pci,guest-cid={},vhostfd=42", guest_cid),
        ]);
    }

    // Add SMBIOS credentials for systemd credential passing
    for credential in &config.smbios_credentials {
        cmd.args(["-smbios", &format!("type=11,value={}", credential)]);
    }

    // Add extra credentials passed to this function
    for credential in extra_credentials {
        cmd.args(["-smbios", &format!("type=11,value={}", credential)]);
    }

    // Configure stdio based on display mode
    match &config.display_mode {
        DisplayMode::Console => {
            // Keep stdio for console interaction
        }
        _ => {
            // Redirect stdout/stderr for non-console modes
            if !config.enable_console {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
    }

    cmd.spawn().context("Failed to spawn QEMU")
}

struct VsockCopier {
    port: VsockAddr,
    #[allow(dead_code)]
    copier: std::thread::JoinHandle<Result<()>>,
}

pub struct RunningQemu {
    pub qemu_process: Child,
    pub virtiofsd_processes: Vec<tokio::process::Child>,
    sd_notification: Option<VsockCopier>,
}

impl RunningQemu {
    /// Spawn QEMU with optional AF_VSOCK debugging enabled
    pub async fn spawn(mut config: QemuConfig) -> Result<Self> {
        let vsockdata = if !config.disable_vsock {
            // Get a unique guest CID using dynamic allocation
            Some(allocate_vsock_cid()?)
        } else {
            None
        };

        let sd_notification = if let Some(target) = config.systemd_notify.take() {
            let vsock = socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )
            .map_err(|e| eyre!("Failed to create AF_VSOCK stream socket: {}", e))?;

            // Bind to host address with ANY port - let kernel allocate a free port
            let addr = VsockAddr::new(VMADDR_CID_ANY, VMADDR_PORT_ANY);
            bind(vsock.as_raw_fd(), &addr)
                .map_err(|e| eyre!("Failed to bind AF_VSOCK stream socket: {}", e))?;

            let port = getsockname(vsock.as_raw_fd())?;
            debug!("Listening on AF_VSOCK {port}");

            // Start listening before spawning the thread
            nix::sys::socket::listen(&vsock, nix::sys::socket::Backlog::new(5).unwrap())
                .map_err(|e| eyre!("Failed to listen on AF_VSOCK: {}", e))?;

            let copier = std::thread::spawn(move || -> Result<()> {
                use std::io::Write;

                debug!("AF_VSOCK listener thread started, waiting for systemd notifications");
                let mut target = target;

                // Accept connections and copy data to target file
                loop {
                    match accept(vsock.as_raw_fd()) {
                        Ok(client_fd) => {
                            debug!("Accepted systemd notification connection");

                            // Read from socket and write to file
                            let mut buffer = [0u8; 4096];
                            match nix::sys::socket::recv(
                                client_fd,
                                &mut buffer,
                                nix::sys::socket::MsgFlags::empty(),
                            ) {
                                Ok(bytes_read) if bytes_read > 0 => {
                                    let data = &buffer[..bytes_read];
                                    debug!("Received systemd notification ({} bytes)", bytes_read);

                                    // Write raw data directly to target file
                                    target.write_all(data)?;
                                    target.write_all(b"\n")?; // Add newline to separate notifications
                                    target.flush()?;
                                }
                                Ok(_) => {
                                    debug!("Connection closed");
                                }
                                Err(e) => {
                                    warn!("Failed to receive data: {}", e);
                                }
                            }

                            // Close client connection
                            let _ = nix::unistd::close(client_fd);
                        }
                        Err(nix::errno::Errno::EAGAIN) => {
                            // No connection available, sleep briefly
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => {
                            warn!("Failed to accept connection: {}", e);
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            });

            Some(VsockCopier { port, copier })
        } else {
            None
        };

        let creds = sd_notification
            .as_ref()
            .map(|sd| {
                let cred = crate::sshcred::smbios_cred_for_vsock_notify(2, sd.port.port());
                vec![cred]
            })
            .unwrap_or_default();

        // Spawn all virtiofsd processes first
        let mut virtiofsd_processes = Vec::new();

        // Spawn main virtiofsd if configured
        if let Some(ref main_config) = config.main_virtiofs_config {
            debug!("Spawning main virtiofsd for: {:?}", main_config.socket_path);
            let process = spawn_virtiofsd_async(main_config).await?;
            virtiofsd_processes.push(process);
            // Wait for socket to be ready before proceeding
            wait_for_virtiofsd_socket(&main_config.socket_path, Duration::from_secs(10)).await?;
        }

        // Spawn additional virtiofsd processes
        for virtiofs_config in &config.virtiofs_configs {
            debug!("Spawning virtiofsd for: {:?}", virtiofs_config.socket_path);
            let process = spawn_virtiofsd_async(virtiofs_config).await?;
            virtiofsd_processes.push(process);

            // Wait for socket to be ready before proceeding
            wait_for_virtiofsd_socket(&virtiofs_config.socket_path, Duration::from_secs(10))
                .await?;
        }

        // Spawn QEMU process with additional VSOCK credential if needed
        let qemu_process = spawn(&config, &creds, vsockdata)?;

        Ok(Self {
            qemu_process,
            virtiofsd_processes,
            sd_notification,
        })
    }

    /// Add a virtiofsd process to be managed by this QEMU instance
    pub fn add_virtiofsd_process(&mut self, process: tokio::process::Child) {
        self.virtiofsd_processes.push(process);
    }

    /// Wait for QEMU process to exit
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        let r = self.qemu_process.wait()?;
        Ok(r)
    }
}

/// Spawn QEMU with automatic process cleanup via guard.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtio_serial_device_creation() {
        let mut config = QemuConfig::new_direct_boot(
            1024,
            1,
            "/test/kernel".to_string(),
            "/test/initramfs".to_string(),
            "/test/socket".to_string(),
        );
        config
            .add_virtio_serial_out("serial0".to_string(), "/tmp/output.txt".to_string())
            .set_kernel_cmdline(vec!["console=ttyS0".to_string()])
            .set_console(true);

        // Test that the config is created correctly
        assert_eq!(config.virtio_serial_devices.len(), 1);
        assert_eq!(config.virtio_serial_devices[0].name, "serial0");
        assert_eq!(
            config.virtio_serial_devices[0].output_file,
            "/tmp/output.txt"
        );
    }

    #[test]
    fn test_virtio_blk_device_creation() {
        let mut config = QemuConfig::new_disk_boot(1024, 1, "/tmp/boot.img".to_string());
        config
            .add_virtio_blk_device("/tmp/test.img".to_string(), "output".to_string())
            .set_console(true);

        // Test that the config is created correctly
        assert_eq!(config.virtio_blk_devices.len(), 1);
        assert_eq!(config.virtio_blk_devices[0].disk_file, "/tmp/test.img");
        assert_eq!(config.virtio_blk_devices[0].serial, "output");
    }

    #[test]
    fn test_disk_boot_config() {
        let mut config = QemuConfig::new_disk_boot(2048, 2, "/tmp/disk.img".to_string());
        config.set_uefi_boot(true).set_console(false);

        if let BootMode::DiskBoot { primary_disk, uefi } = &config.boot_mode {
            assert_eq!(primary_disk, "/tmp/disk.img");
            assert_eq!(*uefi, true);
        } else {
            panic!("Expected DiskBoot mode");
        }

        assert_eq!(config.memory_mb, 2048);
        assert_eq!(config.vcpus, 2);
        assert_eq!(config.enable_console, false);
    }
}

/// VirtiofsD daemon configuration.
/// Cache modes: always(default)/auto/none. Sandbox: none(default)/namespace/chroot.
#[derive(Debug, Clone)]
pub struct VirtiofsConfig {
    /// Unix socket for QEMU communication
    pub socket_path: String,
    /// Host directory to share
    pub shared_dir: String,
    /// Cache mode: always/auto/none
    pub cache_mode: String,
    /// Sandbox: none/namespace/chroot
    pub sandbox: String,
    pub debug: bool,
}

impl Default for VirtiofsConfig {
    fn default() -> Self {
        Self {
            socket_path: "/run/inner-shared/virtiofs.sock".to_string(),
            shared_dir: "/run/source-image".to_string(),
            cache_mode: "always".to_string(),
            sandbox: "none".to_string(),
            debug: false,
        }
    }
}

/// Spawn virtiofsd daemon process as tokio::process::Child.
/// Searches for binary in /usr/libexec, /usr/bin, /usr/local/bin.
/// Creates socket directory if needed, redirects output unless debug=true.
pub async fn spawn_virtiofsd_async(config: &VirtiofsConfig) -> Result<tokio::process::Child> {
    // Validate configuration
    validate_virtiofsd_config(config)?;

    // Try common virtiofsd binary locations
    let virtiofsd_paths = [
        "/usr/libexec/virtiofsd",
        "/usr/bin/virtiofsd",
        "/usr/local/bin/virtiofsd",
    ];

    let virtiofsd_binary = virtiofsd_paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .ok_or_else(|| {
            eyre!(
                "virtiofsd binary not found. Searched paths: {}. Please install virtiofsd.",
                virtiofsd_paths.join(", ")
            )
        })?;

    let mut cmd = tokio::process::Command::new(virtiofsd_binary);
    // SAFETY: This API is safe to call in a forked child.
    unsafe {
        cmd.pre_exec(|| {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))
                .map_err(Into::into)
        });
    }
    cmd.args([
        "--socket-path",
        &config.socket_path,
        "--shared-dir",
        &config.shared_dir,
        "--cache",
        &config.cache_mode,
        "--sandbox",
        &config.sandbox,
    ]);

    // Redirect stdout/stderr to /dev/null unless debug mode is enabled
    if !config.debug {
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    } else {
        // In debug mode, prefix output to distinguish from QEMU
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
    }

    let child = cmd.spawn().with_context(|| {
        format!(
            "Failed to spawn virtiofsd. Binary: {}, Socket: {}, Shared dir: {}",
            virtiofsd_binary, config.socket_path, config.shared_dir
        )
    })?;

    debug!(
        "Spawned virtiofsd: binary={}, socket={}, shared_dir={}, debug={}",
        virtiofsd_binary, config.socket_path, config.shared_dir, config.debug
    );

    Ok(child)
}

/// Spawn virtiofsd daemon process.
/// Searches for binary in /usr/libexec, /usr/bin, /usr/local/bin.
/// Creates socket directory if needed, redirects output unless debug=true.
pub fn spawn_virtiofsd(config: &VirtiofsConfig) -> Result<Child> {
    // Validate configuration
    validate_virtiofsd_config(config)?;

    // Try common virtiofsd binary locations
    let virtiofsd_paths = [
        "/usr/libexec/virtiofsd",
        "/usr/bin/virtiofsd",
        "/usr/local/bin/virtiofsd",
    ];

    let virtiofsd_binary = virtiofsd_paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .ok_or_else(|| {
            eyre!(
                "virtiofsd binary not found. Searched paths: {}. Please install virtiofsd.",
                virtiofsd_paths.join(", ")
            )
        })?;

    let mut cmd = Command::new(virtiofsd_binary);
    cmd.args([
        "--socket-path",
        &config.socket_path,
        "--shared-dir",
        &config.shared_dir,
        "--cache",
        &config.cache_mode,
        "--sandbox",
        &config.sandbox,
    ]);

    // Redirect stdout/stderr to /dev/null unless debug mode is enabled
    if !config.debug {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    } else {
        // In debug mode, prefix output to distinguish from QEMU
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }

    let child = cmd.spawn().with_context(|| {
        format!(
            "Failed to spawn virtiofsd. Binary: {}, Socket: {}, Shared dir: {}",
            virtiofsd_binary, config.socket_path, config.shared_dir
        )
    })?;

    debug!(
        "Spawned virtiofsd: binary={}, socket={}, shared_dir={}, debug={}",
        virtiofsd_binary, config.socket_path, config.shared_dir, config.debug
    );

    Ok(child)
}

/// Wait for virtiofsd socket to become available.
/// Polls every 100ms until socket exists or timeout.
pub async fn wait_for_virtiofsd_socket(socket_path: &str, timeout: Duration) -> Result<()> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if std::path::Path::new(socket_path).exists() {
            debug!("Virtiofsd socket ready: {}", socket_path);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Err(eyre!(
        "Timeout waiting for virtiofsd socket: {}",
        socket_path
    ))
}

/// Validate virtiofsd configuration.
/// Checks shared directory exists/readable, socket path valid,
/// and cache/sandbox modes are valid values.
pub fn validate_virtiofsd_config(config: &VirtiofsConfig) -> Result<()> {
    // Validate shared directory
    let shared_path = std::path::Path::new(&config.shared_dir);
    if !shared_path.exists() {
        return Err(eyre!(
            "Virtiofsd shared directory does not exist: {}",
            config.shared_dir
        ));
    }

    if !shared_path.is_dir() {
        return Err(eyre!(
            "Virtiofsd shared directory is not a directory: {}",
            config.shared_dir
        ));
    }

    // Check if directory is readable
    match std::fs::read_dir(shared_path) {
        Ok(_) => {}
        Err(e) => {
            return Err(eyre!(
                "Cannot read virtiofsd shared directory {}: {}",
                config.shared_dir,
                e
            ));
        }
    }

    // Validate socket path
    if config.socket_path.is_empty() {
        return Err(eyre!("Virtiofsd socket path cannot be empty"));
    }

    let socket_path = std::path::Path::new(&config.socket_path);
    if let Some(socket_dir) = socket_path.parent() {
        if !socket_dir.exists() {
            std::fs::create_dir_all(socket_dir).with_context(|| {
                format!(
                    "Failed to create socket directory: {}",
                    socket_dir.display()
                )
            })?;
        }
    }

    // Validate cache mode
    let valid_cache_modes = ["none", "auto", "always"];
    if !valid_cache_modes.contains(&config.cache_mode.as_str()) {
        return Err(eyre!(
            "Invalid virtiofsd cache mode: '{}'. Valid options: {}",
            config.cache_mode,
            valid_cache_modes.join(", ")
        ));
    }

    // Validate sandbox mode
    let valid_sandbox_modes = ["namespace", "chroot", "none"];
    if !valid_sandbox_modes.contains(&config.sandbox.as_str()) {
        return Err(eyre!(
            "Invalid virtiofsd sandbox mode: '{}'. Valid options: {}",
            config.sandbox,
            valid_sandbox_modes.join(", ")
        ));
    }

    Ok(())
}
