//! Podman-bootc drop-in replacement implementation
//!
//! Provides `bcvk pb` commands that mirror podman-bootc functionality
//! while leveraging our existing libvirt and QEMU infrastructure.

mod domain_list;

use std::path::Path;

// Re-export everything from the main module
pub use self::domain_list::*;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::Context, Result};

/// Podman-bootc drop-in replacement commands
#[derive(Parser)]
pub struct PodmanBootcOpts {
    #[command(subcommand)]
    pub command: PodmanBootcCommand,
}

/// Available podman-bootc commands
#[derive(Subcommand)]
pub enum PodmanBootcCommand {
    /// Run a bootable container as a persistent VM
    Run(RunOpts),
    /// SSH into a running podman-bootc VM
    Ssh(SshOpts),
    /// List all podman-bootc VMs
    List(ListOpts),
    /// Stop a running VM
    Stop(StopOpts),
    /// Start a stopped VM
    Start(StartOpts),
    /// Remove a VM and its resources
    Remove(RemoveOpts),
    /// Show detailed information about a VM
    Inspect(InspectOpts),
}

/// Options for creating and running a bootable container VM
#[derive(Parser)]
pub struct RunOpts {
    /// Container image to run as a bootable VM
    pub image: String,

    /// Name for the VM (auto-generated if not specified)
    #[clap(long)]
    pub name: Option<String>,

    /// Memory size in MB for the VM
    #[clap(long, default_value = "2048")]
    pub memory: u32,

    /// Number of virtual CPUs for the VM
    #[clap(long, default_value = "2")]
    pub cpus: u32,

    /// Disk size in GB for the VM
    #[clap(long, default_value = "20")]
    pub disk_size: u32,

    /// Root filesystem type for installation
    #[clap(long, default_value = "ext4")]
    pub filesystem: String,

    /// Port mapping from host to VM
    #[clap(long = "port", short = 'p', action = clap::ArgAction::Append)]
    pub port_mappings: Vec<String>,

    /// Volume mount from host to VM
    #[clap(long = "volume", short = 'v', action = clap::ArgAction::Append)]
    pub volumes: Vec<String>,

    /// Network mode for the VM
    #[clap(long, default_value = "user")]
    pub network: String,

    /// Keep the VM running in background after creation
    #[clap(long)]
    pub detach: bool,

    /// Automatically SSH into the VM after creation
    #[clap(long)]
    pub ssh: bool,
}

/// Options for SSH connection to a VM
#[derive(Parser)]
pub struct SshOpts {
    /// Name of the VM to connect to
    pub name: String,

    /// Command to execute in the VM
    #[clap(long)]
    pub command: Option<String>,

    /// Additional SSH arguments
    #[clap(allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Options for listing VMs
#[derive(Parser)]
pub struct ListOpts {
    /// Output format
    #[clap(long, default_value = "table")]
    pub format: String,

    /// Show all VMs including stopped ones
    #[clap(long, short = 'a')]
    pub all: bool,
}

/// Options for stopping a VM
#[derive(Parser)]
pub struct StopOpts {
    /// Name of the VM to stop
    pub name: String,

    /// Force stop the VM
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Timeout in seconds for graceful shutdown
    #[clap(long, default_value = "60")]
    pub timeout: u32,
}

/// Options for starting a VM
#[derive(Parser)]
pub struct StartOpts {
    /// Name of the VM to start
    pub name: String,

    /// Automatically SSH into the VM after starting
    #[clap(long)]
    pub ssh: bool,
}

/// Options for removing a VM
#[derive(Parser)]
pub struct RemoveOpts {
    /// Name of the VM to remove
    pub name: String,

    /// Force removal without confirmation
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Remove VM even if it's running
    #[clap(long)]
    pub stop: bool,
}

/// Options for inspecting a VM
#[derive(Parser)]
pub struct InspectOpts {
    /// Name of the VM to inspect
    pub name: String,

    /// Output format
    #[clap(long, default_value = "yaml")]
    pub format: String,
}

impl PodmanBootcOpts {
    /// Execute the podman-bootc command
    pub fn run(self) -> Result<()> {
        match self.command {
            PodmanBootcCommand::Run(opts) => run_vm(opts),
            PodmanBootcCommand::Ssh(opts) => ssh_vm(opts),
            PodmanBootcCommand::List(opts) => list_vms(opts),
            PodmanBootcCommand::Stop(opts) => stop_vm(opts),
            PodmanBootcCommand::Start(opts) => start_vm(opts),
            PodmanBootcCommand::Remove(opts) => remove_vm(opts),
            PodmanBootcCommand::Inspect(opts) => inspect_vm(opts),
        }
    }
}

/// Get the path of the default libvirt storage pool
fn get_libvirt_storage_pool_path() -> Result<std::path::PathBuf> {
    use std::path::PathBuf;
    use std::process::Command;

    // Try user session first (qemu:///session)
    let output = Command::new("virsh")
        .args(&["-c", "qemu:///session", "pool-dumpxml", "default"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => {
            // Try system session (qemu:///system)
            Command::new("virsh")
                .args(&["-c", "qemu:///system", "pool-dumpxml", "default"])
                .output()
                .with_context(|| "Failed to query libvirt storage pool")?
        }
    };

    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!(
            "Failed to get default storage pool info"
        ));
    }

    let xml = String::from_utf8(output.stdout).with_context(|| "Invalid UTF-8 in virsh output")?;

    // Extract path from XML
    // Looking for: <path>/some/path</path>
    let start_tag = "<path>";
    let end_tag = "</path>";

    if let Some(start_pos) = xml.find(start_tag) {
        let start = start_pos + start_tag.len();
        if let Some(end_pos) = xml[start..].find(end_tag) {
            let path_str = &xml[start..start + end_pos];
            return Ok(PathBuf::from(path_str.trim()));
        }
    }

    Err(color_eyre::eyre::eyre!(
        "Could not find path in storage pool XML"
    ))
}

/// Generate a unique VM name from an image name
fn generate_unique_vm_name(image: &str, existing_domains: &[String]) -> String {
    // Extract image name from full image path
    let base_name = if let Some(last_slash) = image.rfind('/') {
        &image[last_slash + 1..]
    } else {
        image
    };

    // Remove tag if present
    let base_name = if let Some(colon) = base_name.find(':') {
        &base_name[..colon]
    } else {
        base_name
    };

    // Sanitize name (replace invalid characters with hyphens)
    let sanitized: String = base_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Find unique name by appending numbers
    let mut candidate = sanitized.clone();
    let mut counter = 1;

    while existing_domains.contains(&candidate) {
        counter += 1;
        candidate = format!("{}-{}", sanitized, counter);
    }

    candidate
}

/// Create disk path for a VM using image hash as suffix
fn create_disk_path(vm_name: &str, source_image: &str) -> Result<Utf8PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::path::PathBuf;

    // Query libvirt for the default storage pool path
    let base_dir = get_libvirt_storage_pool_path().unwrap_or_else(|_| {
        // Fallback to standard paths if we can't query libvirt
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local/share/libvirt/images")
        } else {
            PathBuf::from("/var/lib/libvirt/images")
        }
    });

    // Ensure the directory exists
    fs::create_dir_all(&base_dir)
        .with_context(|| format!("Failed to create directory: {:?}", base_dir))?;

    // Generate a hash of the source image for uniqueness
    let mut hasher = DefaultHasher::new();
    source_image.hash(&mut hasher);
    let image_hash = hasher.finish();
    let hash_prefix = format!("{:x}", image_hash)
        .chars()
        .take(8)
        .collect::<String>();

    // Try to find a unique filename
    let mut counter = 0;
    loop {
        let disk_name = if counter == 0 {
            format!("{}-{}.raw", vm_name, hash_prefix)
        } else {
            format!("{}-{}-{}.raw", vm_name, hash_prefix, counter)
        };

        let disk_path: Utf8PathBuf = base_dir.join(&disk_name).try_into().unwrap();

        // Check if file exists
        if !disk_path.exists() {
            return Ok(disk_path);
        }

        counter += 1;
        if counter > 100 {
            return Err(color_eyre::eyre::eyre!(
                "Could not create unique disk path after 100 attempts"
            ));
        }
    }
}

/// Create and run a bootable container VM
///
/// This function now delegates to the actual implementation
/// to maintain compatibility while centralizing the functionality.
pub fn run_vm(opts: RunOpts) -> Result<()> {
    run_vm_impl(opts)
}

/// Create and run a bootable container VM (original implementation)
///
/// This is the original implementation that is now called from both
/// `bcvk pb run` and `bcvk libvirt run`.
pub fn run_vm_impl(opts: RunOpts) -> Result<()> {
    use crate::install_options::InstallOptions;
    use crate::run_ephemeral::CommonVmOpts;
    use crate::to_disk::ToDiskOpts;

    let lister = DomainLister::new();
    let existing_domains = lister
        .list_all_domains()
        .with_context(|| "Failed to list existing domains")?;

    // Generate or validate VM name
    let vm_name = match &opts.name {
        Some(name) => {
            if existing_domains.contains(name) {
                return Err(color_eyre::eyre::eyre!("VM '{}' already exists", name));
            }
            name.clone()
        }
        None => generate_unique_vm_name(&opts.image, &existing_domains),
    };

    println!("Creating VM '{}' from image '{}'...", vm_name, opts.image);

    // Create disk path in the standard libvirt images directory
    let disk_path =
        create_disk_path(&vm_name, &opts.image).with_context(|| "Failed to create disk path")?;

    // Phase 1: Create bootable disk image using to_disk
    println!("📀 Creating bootable disk image...");

    let to_disk_opts = ToDiskOpts {
        source_image: opts.image.clone(),
        target_disk: disk_path.clone(),
        disk_size: Some((opts.disk_size as u64) * 1024 * 1024 * 1024), // Convert GB to bytes
        install: InstallOptions {
            filesystem: Some(opts.filesystem.clone()),
            root_size: None,
            storage_path: None,
        },
        common: CommonVmOpts {
            memory: Some(format!("{}M", opts.memory)),
            vcpus: opts.cpus,
            kernel_args: vec![],
            net: None,
            console: false,
            debug: false,
            virtio_serial_out: vec![],
            execute: vec![],
            ssh_keygen: true, // Enable SSH key generation
        },
        label: vec![],
    };

    // Run the disk creation
    crate::to_disk::run(to_disk_opts).with_context(|| "Failed to create bootable disk image")?;

    println!("✅ Disk image created at: {}", disk_path);

    // Phase 2: Create libvirt domain
    println!("🖥️  Creating libvirt domain...");

    // Create the domain directly (simpler than using libvirt/create for files)
    create_libvirt_domain_from_disk(&vm_name, disk_path.as_std_path(), &opts)
        .with_context(|| "Failed to create libvirt domain")?;

    // VM is now managed by libvirt, no need to track separately

    println!("✅ VM '{}' created successfully!", vm_name);
    println!("  Image: {}", opts.image);
    println!("  Disk: {}", disk_path);
    println!("  Memory: {} MB", opts.memory);
    println!("  CPUs: {}", opts.cpus);

    if opts.ssh {
        ssh_vm(SshOpts {
            name: vm_name,
            command: None,
            args: Default::default(),
        })
    } else {
        println!("\n🔗 Use 'bcvk pb ssh {}' to connect", vm_name);
        Ok(())
    }
}

/// Find an available SSH port for port forwarding using random allocation
fn find_available_ssh_port() -> u16 {
    use rand::Rng;

    // Try random ports in the range 2222-3000 to avoid conflicts in concurrent scenarios
    let mut rng = rand::rng();
    const PORT_RANGE_START: u16 = 2222;
    const PORT_RANGE_END: u16 = 3000;

    // Try up to 100 random attempts
    for _ in 0..100 {
        let port = rng.random_range(PORT_RANGE_START..PORT_RANGE_END);
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }

    // Fallback to sequential search if random allocation fails
    for port in PORT_RANGE_START..PORT_RANGE_END {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }

    PORT_RANGE_START // Ultimate fallback
}

/// Create a libvirt domain directly from a disk image file
fn create_libvirt_domain_from_disk(
    domain_name: &str,
    disk_path: &Path,
    opts: &RunOpts,
) -> Result<()> {
    use crate::libvirt::domain::DomainBuilder;
    use crate::ssh::generate_ssh_keypair;
    use crate::sshcred::smbios_cred_for_root_ssh;
    use std::process::Command;
    use tracing::info;

    // Generate SSH keypair for the domain
    info!(
        "Generating ephemeral SSH keypair for domain '{}'",
        domain_name
    );

    // Find available SSH port for this domain
    let ssh_port = find_available_ssh_port();
    info!(
        "Allocated SSH port {} for domain '{}'",
        ssh_port, domain_name
    );

    // Use temporary files for key generation, then read content and clean up
    let temp_dir = tempfile::tempdir()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to create temporary directory: {}", e))?;

    // Generate keypair
    let keypair = generate_ssh_keypair(temp_dir.path(), "id_rsa")?;

    // Read the key contents from the generated keypair
    let private_key_content = std::fs::read_to_string(&keypair.private_key_path)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to read generated private key: {}", e))?;
    let public_key_content = std::fs::read_to_string(&keypair.public_key_path)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to read generated public key: {}", e))?;

    let private_key_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        private_key_content.as_bytes(),
    );
    info!("Generated ephemeral SSH keypair (will be stored in domain XML)");

    // Generate SMBIOS credential for SSH key injection
    let smbios_cred = smbios_cred_for_root_ssh(&public_key_content)?;

    // Build domain XML using the existing DomainBuilder with bootc metadata and SSH keys
    let domain_xml = DomainBuilder::new()
        .with_name(domain_name)
        .with_memory(opts.memory as u64)
        .with_vcpus(opts.cpus)
        .with_disk(&disk_path.to_string_lossy())
        .with_network("none") // Use QEMU args for SSH networking instead
        .with_metadata("bootc:source-image", &opts.image)
        .with_metadata("bootc:memory-mb", &opts.memory.to_string())
        .with_metadata("bootc:vcpus", &opts.cpus.to_string())
        .with_metadata("bootc:disk-size-gb", &opts.disk_size.to_string())
        .with_metadata("bootc:filesystem", &opts.filesystem)
        .with_metadata("bootc:network", &opts.network)
        .with_metadata("bootc:ssh-generated", "true")
        .with_metadata("bootc:ssh-private-key-base64", &private_key_base64)
        .with_metadata("bootc:ssh-port", &ssh_port.to_string())
        .with_qemu_args(vec![
            "-smbios".to_string(),
            format!("type=11,value={}", smbios_cred),
            "-netdev".to_string(),
            format!("user,id=ssh0,hostfwd=tcp::{}-:22", ssh_port),
            "-device".to_string(),
            "virtio-net-pci,netdev=ssh0,addr=0x3".to_string(),
        ])
        .build_xml()
        .with_context(|| "Failed to build domain XML")?;

    // Write XML to temporary file
    let xml_path = format!("/tmp/{}.xml", domain_name);
    std::fs::write(&xml_path, domain_xml).with_context(|| "Failed to write domain XML")?;

    // Define the domain
    let output = Command::new("virsh")
        .args(&["define", &xml_path])
        .output()
        .with_context(|| "Failed to run virsh define")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to define libvirt domain: {}",
            stderr
        ));
    }

    // Start the domain by default (podman-bootc compatibility)
    let output = Command::new("virsh")
        .args(&["start", domain_name])
        .output()
        .with_context(|| "Failed to start domain")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to start libvirt domain: {}",
            stderr
        ));
    }

    // Clean up temporary XML file
    let _ = std::fs::remove_file(&xml_path);

    Ok(())
}

/// SSH into a running VM
pub fn ssh_vm(opts: SshOpts) -> Result<()> {
    // Use libvirt as the source of truth for domain lookup
    let lister = DomainLister::new();

    // Verify the domain exists and is running
    let domains = lister
        .list_bootc_domains()
        .with_context(|| "Failed to list bootc domains from libvirt")?;

    let vm = domains
        .iter()
        .find(|d| d.name == opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if !vm.is_running() {
        return Err(color_eyre::eyre::eyre!(
            "VM '{}' is not running (status: {})",
            vm.name,
            vm.status_string()
        ));
    }

    // Delegate to the existing libvirt SSH functionality
    let mut command = Vec::new();

    // Handle the command and args from podman-bootc style
    if let Some(cmd) = opts.command {
        command.push(cmd);
    }
    command.extend(opts.args);

    let ssh_opts = crate::libvirt::ssh::LibvirtSshOpts {
        domain_name: vm.name.clone(),
        connect: None,
        user: "root".to_string(),
        command,
        strict_host_keys: false,
        timeout: 30,
    };

    crate::libvirt::ssh::run(ssh_opts)
}

/// List all VMs
pub fn list_vms(opts: ListOpts) -> Result<()> {
    // Use libvirt as the source of truth for domain listing
    let lister = DomainLister::new();

    let domains = if opts.all {
        lister
            .list_bootc_domains()
            .with_context(|| "Failed to list bootc domains from libvirt")?
    } else {
        lister
            .list_running_bootc_domains()
            .with_context(|| "Failed to list running bootc domains from libvirt")?
    };

    match opts.format.as_str() {
        "table" => {
            if domains.is_empty() {
                if opts.all {
                    println!("No VMs found");
                    println!("Tip: Create VMs with 'bcvk pb run <image>'");
                } else {
                    println!("No running VMs found");
                    println!("Use --all to see stopped VMs or 'bcvk pb run <image>' to create one");
                }
                return Ok(());
            }
            println!(
                "{:<20} {:<40} {:<12} {:<20}",
                "NAME", "IMAGE", "STATUS", "MEMORY"
            );
            println!("{}", "=".repeat(92));
            for domain in &domains {
                let image = match &domain.image {
                    Some(img) => {
                        if img.len() > 38 {
                            format!("{}...", &img[..35])
                        } else {
                            img.clone()
                        }
                    }
                    None => "<no metadata>".to_string(),
                };
                let memory = match domain.memory_mb {
                    Some(mem) => format!("{}MB", mem),
                    None => "unknown".to_string(),
                };
                println!(
                    "{:<20} {:<40} {:<12} {:<20}",
                    domain.name,
                    image,
                    domain.status_string(),
                    memory
                );
            }
            println!(
                "\nFound {} domain{} (source: libvirt)",
                domains.len(),
                if domains.len() == 1 { "" } else { "s" }
            );
        }
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&domains)
                    .with_context(|| "Failed to serialize domains as JSON")?
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

/// Stop a running VM
pub fn stop_vm(opts: StopOpts) -> Result<()> {
    use std::process::Command;

    let lister = DomainLister::new();

    // Check if domain exists and get its state
    let state = lister
        .get_domain_state(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if state != "running" {
        println!("VM '{}' is already stopped (state: {})", opts.name, state);
        return Ok(());
    }

    println!("🛑 Stopping VM '{}'...", opts.name);

    // Use virsh to stop the domain
    let mut cmd = Command::new("virsh");
    if opts.force {
        cmd.args(&["destroy", &opts.name]);
    } else {
        cmd.args(&["shutdown", &opts.name]);
    }

    let output = cmd
        .output()
        .with_context(|| "Failed to run virsh command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to stop VM '{}': {}",
            opts.name,
            stderr
        ));
    }

    // VM status is now managed by libvirt

    println!("✅ VM '{}' stopped successfully", opts.name);
    Ok(())
}

/// Start a stopped VM
pub fn start_vm(opts: StartOpts) -> Result<()> {
    use std::process::Command;

    let lister = DomainLister::new();

    // Check if domain exists and get its state
    let state = lister
        .get_domain_state(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if state == "running" {
        println!("VM '{}' is already running", opts.name);
        if opts.ssh {
            println!("🔗 Connecting to running VM...");
            // TODO: SSH to already running VM
        }
        return Ok(());
    }

    println!("🚀 Starting VM '{}'...", opts.name);

    // Use virsh to start the domain
    let output = Command::new("virsh")
        .args(&["start", &opts.name])
        .output()
        .with_context(|| "Failed to run virsh start")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to start VM '{}': {}",
            opts.name,
            stderr
        ));
    }

    // VM status is now managed by libvirt

    println!("✅ VM '{}' started successfully", opts.name);

    if opts.ssh {
        println!("🔗 Use 'bcvk pb ssh {}' to connect", opts.name);
    }

    Ok(())
}

/// Remove a VM and its resources
pub fn remove_vm(opts: RemoveOpts) -> Result<()> {
    use std::process::Command;

    let lister = DomainLister::new();

    // Check if domain exists and get its state
    let state = lister
        .get_domain_state(&opts.name)
        .map_err(|_| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    // Get domain info for display
    let domain_info = lister
        .get_domain_info(&opts.name)
        .with_context(|| format!("Failed to get info for VM '{}'", opts.name))?;

    // Check if VM is running
    if state == "running" {
        if opts.stop {
            println!("🛑 Stopping running VM '{}'...", opts.name);
            let output = Command::new("virsh")
                .args(&["destroy", &opts.name])
                .output()
                .with_context(|| "Failed to stop VM before removal")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(color_eyre::eyre::eyre!(
                    "Failed to stop VM '{}' before removal: {}",
                    opts.name,
                    stderr
                ));
            }
        } else {
            return Err(color_eyre::eyre::eyre!(
                "VM '{}' is running. Use --stop to force removal or stop it first.",
                opts.name
            ));
        }
    }

    // Confirmation prompt
    if !opts.force {
        println!(
            "This will permanently delete VM '{}' and its data:",
            opts.name
        );
        if let Some(ref image) = domain_info.image {
            println!("  Image: {}", image);
        }
        if let Some(ref disk_path) = domain_info.disk_path {
            println!("  Disk: {}", disk_path);
        }
        println!("  Status: {}", domain_info.status_string());
        println!();
        println!("Are you sure? This cannot be undone. Use --force to skip this prompt.");
        return Ok(());
    }

    println!("🗑️  Removing VM '{}'...", opts.name);

    // Remove libvirt domain
    println!("  Removing libvirt domain...");
    let output = Command::new("virsh")
        .args(&["undefine", &opts.name, "--remove-all-storage"])
        .output()
        .with_context(|| "Failed to undefine libvirt domain")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!(
            "Failed to remove libvirt domain: {}",
            stderr
        ));
    }

    println!("✅ VM '{}' removed successfully", opts.name);
    Ok(())
}

/// Show detailed information about a VM
pub fn inspect_vm(opts: InspectOpts) -> Result<()> {
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
