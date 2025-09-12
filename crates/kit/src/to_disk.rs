//! Install bootc images to disk using ephemeral VMs
//!
//! This module provides the core installation functionality for bcvk, enabling
//! automated installation of bootc container images to disk images through an
//! ephemeral VM-based approach.
//!
//! # Installation Workflow
//!
//! The bootc installation process follows these key steps:
//!
//! 1. **Image Preparation**: Validates the source container image and prepares the
//!    target disk file, creating it with appropriate sizing if it doesn't exist
//!
//! 2. **Storage Configuration**: Sets up container storage access within the
//!    installation VM by mounting the host's container storage as read-only
//!
//! 3. **Ephemeral VM Launch**: Creates a temporary VM using the bootc image itself
//!    as the installation environment, with the target disk attached via virtio-blk
//!
//! 4. **Bootc Installation**: Executes `bootc install to-disk` within the VM,
//!    installing the container image to the attached disk with the specified
//!    filesystem and configuration options
//!
//! 5. **Cleanup**: The ephemeral VM automatically shuts down after installation,
//!    leaving behind the configured disk image ready for deployment
//!
//! # Disk Image Management
//!
//! The installation process creates and manages disk images as follows:
//!
//! - **Automatic Sizing**: Target disk size is calculated as 2x the source image
//!   size with a 4GB minimum to ensure adequate space for installation
//!
//! - **File Creation**: Creates sparse disk image files that grow as needed,
//!   supporting efficient storage usage
//!
//! - **Virtio-blk Attachment**: Attaches the target disk to the VM using virtio-blk
//!   with a predictable device name (`/dev/disk/by-id/virtio-output`)
//!
//! # Filesystem and Storage Options
//!
//! The module supports multiple filesystem types and storage configurations:
//!
//! - **Filesystem Types**: ext4 (default), xfs, and btrfs filesystems
//! - **Custom Root Size**: Optional specification of root filesystem size
//! - **Storage Path Detection**: Automatic detection of host container storage or
//!   manual specification for custom setups
//!
//! # Ephemeral VM Integration
//!
//! This module leverages the ephemeral VM infrastructure (`run_ephemeral`) to:
//!
//! - **Isolated Environment**: Provides a clean, isolated environment for
//!   installation without affecting the host system
//!
//! - **Container Storage Access**: Mounts host container storage read-only to
//!   access the source image without network dependencies
//!
//! - **Automated Lifecycle**: Handles VM startup, installation execution, and
//!   cleanup automatically with proper error handling
//!
//! - **Debug Support**: Provides comprehensive logging and debug output for
//!   troubleshooting installation issues
//!
//! # Usage Examples
//!
//! ```bash
//! # Basic installation with defaults
//! bcvk to-disk quay.io/centos-bootc/centos-bootc:stream10 output.img
//!
//! # Custom filesystem and size
//! bcvk to-disk --filesystem xfs --root-size 20G \
//!     quay.io/centos-bootc/centos-bootc:stream10 output.img
//! ```

use crate::install_options::InstallOptions;
use crate::run_ephemeral::{run_synchronous as run_ephemeral, CommonVmOpts, RunEphemeralOpts};
use crate::{images, utils};
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use std::borrow::Cow;
use std::path::PathBuf;
use tracing::debug;

/// Configuration options for installing a bootc container image to disk
///
/// See the module-level documentation for details on the installation architecture and workflow.
#[derive(Debug, Parser)]
pub struct ToDiskOpts {
    /// Container image to install
    pub source_image: String,

    /// Target disk/device path
    pub target_disk: PathBuf,

    /// Installation options (filesystem, root-size, storage-path)
    #[clap(flatten)]
    pub install: InstallOptions,

    /// Disk size to create (optional, defaults to calculated size based on source image)
    #[clap(long)]
    pub disk_size: Option<u64>,

    /// Common VM configuration options
    #[clap(flatten)]
    pub common: CommonVmOpts,

    #[clap(
        long = "label",
        help = "Add metadata to the container in key=value form"
    )]
    pub label: Vec<String>,
}

impl ToDiskOpts {
    /// Get the container image to use as the installation environment
    ///
    /// Uses the source image itself as the installer environment.
    fn get_installer_image(&self) -> &str {
        &self.source_image
    }

    /// Resolve and validate the container storage path
    ///
    /// Uses explicit storage_path if specified, otherwise auto-detects container storage.
    fn get_storage_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.install.storage_path {
            utils::validate_container_storage_path(path)?;
            Ok(path.to_path_buf())
        } else {
            utils::detect_container_storage_path()
        }
    }

    /// Validate and prepare the target disk image file
    ///
    /// Ensures the target is suitable for use as a disk image and creates parent directories if needed.
    fn prepare_target_disk(&self) -> Result<()> {
        let path = &self.target_disk;

        // Error out if target disk already exists
        if path.exists() {
            return Err(eyre!(
                "Target disk already exists: {:?}. Remove it first or use a different path.",
                path
            ));
        } else {
            // Validate parent directory exists or can be created
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    // Check if we can create the parent directory by attempting to create it
                    // but first check if parent's parent exists
                    if let Some(grandparent) = parent.parent() {
                        if !grandparent.exists() {
                            return Err(eyre!("Parent directory does not exist: {:?}", parent));
                        }
                    }
                }
                std::fs::create_dir_all(parent)?;
            }
        }

        Ok(())
    }

    /// Generate the complete bootc installation command
    fn generate_bootc_install_command(&self) -> Vec<String> {
        let source_imgref = format!("containers-storage:{}", self.source_image);

        let bootc_install = [
            "env",
            // This is the magic trick to pull the storage from the host
            "STORAGE_OPTS=additionalimagestore=/run/virtiofs-mnt-hoststorage/",
            "bootc",
            "install",
            "to-disk",
            // Default to being a generic image here, if someone cares they can override this
            "--generic-image",
            "--source-imgref",
        ]
        .into_iter()
        .map(Cow::Borrowed)
        .chain(std::iter::once(source_imgref.into()))
        .chain(self.install.to_bootc_args().into_iter().map(Cow::Owned))
        .chain(std::iter::once(Cow::Borrowed(
            "/dev/disk/by-id/virtio-output",
        )))
        .fold(String::new(), |mut acc, elt| {
            if !acc.is_empty() {
                acc.push(' ');
            }
            acc.push_str(&*elt);
            acc
        });
        // TODO: make /var a tmpfs by default (actually make run-ephemeral more like a readonly bootc)
        vec![
            "mount -t tmpfs tmpfs /var/lib/containers".to_owned(),
            bootc_install,
        ]
    }

    /// Calculate the optimal target disk size based on the source image or explicit size
    ///
    /// Returns explicit disk_size if provided, otherwise 2x the image size with a 4GB minimum.
    fn calculate_disk_size(&self) -> Result<u64> {
        if let Some(size) = self.disk_size {
            return Ok(size);
        }

        // Get the image size and multiply by 2 for installation space
        let image_size =
            images::get_image_size(&self.source_image).unwrap_or(2 * 1024 * 1024 * 1024); // Default to 2GB if we can't get image size

        // Minimum 4GB, otherwise 2x the image size
        let disk_size = std::cmp::max(image_size * 2, 4 * 1024 * 1024 * 1024);
        Ok(disk_size)
    }
}

/// Execute a bootc installation using an ephemeral VM
///
/// Main entry point for the bootc installation process. See module-level documentation
/// for details on the installation workflow and architecture.
pub fn run(opts: ToDiskOpts) -> Result<()> {
    // Phase 1: Validation and preparation
    // Ensure target disk path is valid and create parent directories if needed
    opts.prepare_target_disk()?;
    // Resolve container storage path (auto-detect or validate specified path)
    let storage_path = opts.get_storage_path()?;

    // Always output container storage path for test visibility
    eprintln!("Using container storage at: {:?}", storage_path);

    // Debug logging for installation configuration
    if opts.common.debug {
        debug!("Using container storage at: {:?}", storage_path);
        debug!("Installing to target disk: {:?}", opts.target_disk);
        debug!("Filesystem: {:?}", opts.install.filesystem);
        if let Some(ref root_size) = opts.install.root_size {
            debug!("Root size: {}", root_size);
        }
    }

    // Phase 2: Target disk preparation
    // Create sparse disk file with calculated size if it doesn't exist
    if !opts.target_disk.exists() {
        let disk_size = opts.calculate_disk_size()?;
        debug!(
            "Creating target disk file: {:?} (size: {} bytes)",
            opts.target_disk, disk_size
        );

        // Create sparse file - only allocates space as data is written
        let file = std::fs::File::create(&opts.target_disk)?;
        file.set_len(disk_size)?;

        if opts.common.debug {
            println!(
                "Created target disk file: {:?} (size: {} bytes)",
                opts.target_disk, disk_size
            );
        }
    }

    // Phase 3: Installation command generation
    // Generate complete script including storage setup and bootc install
    let bootc_install_command = opts.generate_bootc_install_command();

    // Phase 4: Ephemeral VM configuration
    let common_opts = opts.common.clone();

    // Configure VM for installation:
    // - Use source image as installer environment
    // - Mount host storage read-only for image access
    // - Attach target disk via virtio-blk
    // - Disable networking (using local storage only)
    let ephemeral_opts = RunEphemeralOpts {
        image: opts.get_installer_image().to_string(),
        common: common_opts,
        podman: crate::run_ephemeral::CommonPodmanOptions {
            rm: true, // Clean up container after installation
            label: opts.label,
            ..Default::default()
        },
        bind_mounts: Vec::new(),        // No additional bind mounts needed
        ro_bind_mounts: Vec::new(),     // No additional ro bind mounts needed
        systemd_units_dir: None,        // No custom systemd units
        log_cmdline: opts.common.debug, // Log kernel command line if debug
        bind_storage_ro: true,          // Mount host container storage read-only
        mount_disk_files: vec![format!("{}:output", opts.target_disk.display())], // Attach target disk
    };

    // Phase 5: Final VM configuration and execution
    let mut final_opts = ephemeral_opts;
    // Set the installation script to execute in the VM
    final_opts.common.execute = bootc_install_command;

    // Ensure clean shutdown after installation completes
    final_opts
        .common
        .kernel_args
        .push("systemd.default_target=poweroff.target".to_string());

    // Phase 6: Launch VM and execute installation
    // The ephemeral VM will:
    // 1. Boot using the bootc image
    // 2. Mount host storage and target disk
    // 3. Execute the installation script
    // 4. Shut down automatically after completion
    run_ephemeral(final_opts)
}

// Note: Unit tests should not launch containers, VMs, or perform other system-level operations.
// Integration tests that launch containers/VMs should be placed in the integration-tests crate.
// Unit tests here should only test pure functions and basic validation logic.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_target_disk_file() -> Result<()> {
        use tempfile::TempDir;

        let temp_dir = TempDir::new()?;
        let target_disk = temp_dir.path().join("test-disk.img");

        let opts = RunInstallOpts {
            source_image: "test:latest".to_string(),
            target_disk,
            label: Default::default(),
            install: InstallOptions {
                filesystem: Some("ext4".to_string()),
                root_size: None,
                storage_path: None,
            },
            disk_size: None,
            common: CommonVmOpts::default(),
        };

        opts.prepare_target_disk()?;
        Ok(())
    }

    #[test]
    fn test_calculate_disk_size() -> Result<()> {
        let opts = RunInstallOpts {
            source_image: "test:latest".to_string(),
            target_disk: "/tmp/test.img".into(),
            label: Default::default(),
            install: InstallOptions {
                filesystem: Some("ext4".to_string()),
                root_size: None,
                storage_path: None,
            },
            disk_size: None,
            common: CommonVmOpts::default(),
        };

        let size = opts.calculate_disk_size()?;
        // Should be at least 4GB minimum
        assert!(size >= 4 * 1024 * 1024 * 1024);
        Ok(())
    }
}
