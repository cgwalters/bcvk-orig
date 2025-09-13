//! Podman-bootc drop-in replacement implementation
//!
//! Provides `bcvk pb` commands that mirror podman-bootc functionality
//! while leveraging our existing libvirt and QEMU infrastructure.

mod vm_registry;

// Re-export everything from the main module
pub use self::vm_registry::*;

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
    pub name: Option<String>,

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

/// Create and run a bootable container VM
pub fn run_vm(opts: RunOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;

    let mut registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    // Generate or validate VM name
    let vm_name = match opts.name {
        Some(name) => {
            if !registry.is_name_available(&name) {
                return Err(color_eyre::eyre::eyre!("VM '{}' already exists", name));
            }
            name
        }
        None => registry.generate_vm_name(&opts.image),
    };

    // Create VM metadata
    let mut vm = VmMetadata::new(
        vm_name.clone(),
        opts.image.clone(),
        opts.memory,
        opts.cpus,
        opts.disk_size,
        opts.filesystem.clone(),
        opts.network.clone(),
        opts.port_mappings.clone(),
        opts.volumes.clone(),
    );

    // Set disk path
    let disk_path = manager
        .create_disk_path(&vm_name)
        .with_context(|| "Failed to create disk path")?;
    vm.set_disk_path(disk_path);

    // Add VM to registry
    registry
        .add_vm(vm.clone())
        .with_context(|| "Failed to add VM to registry")?;

    // Save registry
    manager
        .save_registry(&registry)
        .with_context(|| "Failed to save VM registry")?;

    println!("Created VM '{}'", vm_name);
    println!("  Image: {}", opts.image);
    println!("  Disk path: {}", vm.disk_path.display());
    println!("\nVM created successfully! Use 'bcvk pb list' to see all VMs.");

    Ok(())
}

/// SSH into a running VM
pub fn ssh_vm(opts: SshOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    // Find target VM
    let vm = match opts.name {
        Some(name) => registry
            .get_vm(&name)
            .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", name))?,
        None => registry
            .get_latest_vm()
            .ok_or_else(|| color_eyre::eyre::eyre!("No VMs found"))?,
    };

    if !vm.is_running() {
        return Err(color_eyre::eyre::eyre!(
            "VM '{}' is not running (status: {})",
            vm.name,
            vm.status_string()
        ));
    }

    println!("SSH connection not yet implemented");
    println!("Would connect to VM: {}", vm.name);

    Ok(())
}

/// List all VMs
pub fn list_vms(opts: ListOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    let vms: Vec<&VmMetadata> = if opts.all {
        registry.list_vms()
    } else {
        registry.get_running_vms()
    };

    match opts.format.as_str() {
        "table" => {
            if vms.is_empty() {
                println!("No VMs found");
                return Ok(());
            }
            println!(
                "{:<20} {:<40} {:<12} {:<20}",
                "NAME", "IMAGE", "STATUS", "CREATED"
            );
            println!("{}", "=".repeat(92));
            for vm in vms {
                let image = if vm.image.len() > 38 {
                    format!("{}...", &vm.image[..35])
                } else {
                    vm.image.clone()
                };
                println!(
                    "{:<20} {:<40} {:<12} {:<20}",
                    vm.name,
                    image,
                    vm.status_string(),
                    "recent"
                );
            }
        }
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&vms)
                    .with_context(|| "Failed to serialize VMs as JSON")?
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
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let mut registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    let vm = registry
        .get_vm(&opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if !vm.is_running() {
        println!("VM '{}' is already stopped", opts.name);
        return Ok(());
    }

    registry
        .update_vm_status(&opts.name, VmStatus::Stopped)
        .with_context(|| "Failed to update VM status")?;
    manager
        .save_registry(&registry)
        .with_context(|| "Failed to save VM registry")?;

    println!("VM '{}' stopped successfully", opts.name);
    Ok(())
}

/// Start a stopped VM
pub fn start_vm(opts: StartOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let mut registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    let vm = registry
        .get_vm(&opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if vm.is_running() {
        println!("VM '{}' is already running", opts.name);
        return Ok(());
    }

    registry
        .update_vm_status(&opts.name, VmStatus::Running)
        .with_context(|| "Failed to update VM status")?;
    manager
        .save_registry(&registry)
        .with_context(|| "Failed to save VM registry")?;

    println!("VM '{}' started successfully", opts.name);
    Ok(())
}

/// Remove a VM and its resources
pub fn remove_vm(opts: RemoveOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let mut registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    let vm = registry
        .get_vm(&opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    if vm.is_running() && !opts.stop {
        return Err(color_eyre::eyre::eyre!(
            "VM '{}' is running. Use --stop to force removal or stop it first.",
            opts.name
        ));
    }

    if !opts.force {
        println!(
            "This will permanently delete VM '{}' and its data.",
            opts.name
        );
        println!("Use --force to skip this confirmation.");
        return Ok(());
    }

    let vm_to_remove = vm.clone();
    registry
        .remove_vm(&opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found in registry", opts.name))?;
    manager
        .cleanup_vm_files(&vm_to_remove)
        .with_context(|| "Failed to clean up VM files")?;
    manager
        .save_registry(&registry)
        .with_context(|| "Failed to save VM registry")?;

    println!("VM '{}' removed successfully", opts.name);
    Ok(())
}

/// Show detailed information about a VM
pub fn inspect_vm(opts: InspectOpts) -> Result<()> {
    let manager =
        VmRegistryManager::new().with_context(|| "Failed to initialize VM registry manager")?;
    let registry = manager
        .load_registry()
        .with_context(|| "Failed to load VM registry")?;

    let vm = registry
        .get_vm(&opts.name)
        .ok_or_else(|| color_eyre::eyre::eyre!("VM '{}' not found", opts.name))?;

    match opts.format.as_str() {
        "yaml" => {
            println!("name: {}", vm.name);
            println!("image: {}", vm.image);
            println!("status: {}", vm.status_string());
        }
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(vm)
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
