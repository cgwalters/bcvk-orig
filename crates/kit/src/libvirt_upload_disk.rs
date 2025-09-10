//! Upload bootc disk images to libvirt with proper metadata annotations
//!
//! This module provides functionality to upload disk images created by run-install
//! to libvirt storage pools, maintaining container image metadata as libvirt annotations.

use crate::install_options::InstallOptions;
use crate::run_ephemeral::{default_vcpus, DEFAULT_MEMORY_STR};
use crate::run_install::{run as run_install, RunInstallOpts};
use crate::{images, utils};
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

/// Configuration options for uploading a bootc disk image to libvirt
#[derive(Debug, Parser)]
pub struct LibvirtUploadDiskOpts {
    /// Container image to install and upload
    pub source_image: String,

    /// Name for the libvirt volume (defaults to sanitized image name)
    #[clap(long)]
    pub volume_name: Option<String>,

    /// Libvirt storage pool name
    #[clap(long, default_value = "default")]
    pub pool: String,

    /// Size of the disk image (e.g., '20G', '10240M'). If not specified, uses the actual size of the created disk.
    #[clap(long)]
    pub disk_size: Option<String>,

    /// Installation options (filesystem, root-size, storage-path)
    #[clap(flatten)]
    pub install: InstallOptions,

    /// Memory size for installation VM (e.g. 2G, 1024M)
    #[clap(long, default_value = DEFAULT_MEMORY_STR)]
    pub memory: String,

    /// Number of vCPUs for installation VM
    #[clap(long, default_value_t = default_vcpus())]
    pub vcpus: u32,

    /// Additional kernel arguments for installation
    #[clap(long)]
    pub karg: Vec<String>,

    /// Skip uploading to libvirt (useful for testing)
    #[clap(long)]
    pub skip_upload: bool,

    /// Keep temporary disk image after upload
    #[clap(long)]
    pub keep_temp: bool,
}

impl LibvirtUploadDiskOpts {
    /// Generate a sanitized volume name from the container image
    fn get_volume_name(&self) -> String {
        if let Some(ref name) = self.volume_name {
            return name.clone();
        }

        // Sanitize the image name for use as a volume name
        let image_name = self.source_image.clone();

        // Remove registry prefix if present
        let name = image_name
            .split('/')
            .last()
            .unwrap_or(&image_name)
            .replace(':', "-")
            .replace('/', "-")
            .replace('.', "-");

        format!("bootc-{}", name)
    }

    /// Check if libvirt storage pool exists
    fn check_pool_exists(&self) -> Result<()> {
        let output = Command::new("virsh")
            .args(&["pool-info", &self.pool])
            .output()?;

        if !output.status.success() {
            return Err(eyre!(
                "Storage pool '{}' does not exist. Create it with: virsh pool-define-as {} dir - - - - /var/lib/libvirt/images",
                self.pool, self.pool
            ));
        }

        Ok(())
    }

    /// Upload the disk image to libvirt storage pool
    fn upload_to_libvirt(&self, disk_path: &Path, disk_size_bytes: u64) -> Result<()> {
        info!("Uploading disk to libvirt pool '{}'", self.pool);

        // Check pool exists
        self.check_pool_exists()?;

        let volume_name = self.get_volume_name();
        let volume_path = format!("{}.raw", volume_name);

        // Delete existing volume if it exists
        let _ = Command::new("virsh")
            .args(&["vol-delete", &volume_path, "--pool", &self.pool])
            .output();

        // Use the provided disk size
        let output = Command::new("virsh")
            .args(&[
                "vol-create-as",
                &self.pool,
                &volume_path,
                &disk_size_bytes.to_string(),
                "--format",
                "raw",
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to create volume: {}", stderr));
        }

        // Upload the disk image to the volume
        info!("Uploading disk image to volume '{}'", volume_path);
        let output = Command::new("virsh")
            .args(&[
                "vol-upload",
                &volume_path,
                disk_path.to_str().unwrap(),
                "--pool",
                &self.pool,
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to upload volume: {}", stderr));
        }

        Ok(())
    }

    /// Add metadata annotations to the libvirt volume
    fn add_volume_metadata(&self) -> Result<()> {
        let volume_name = self.get_volume_name();
        let volume_path = format!("{}.raw", volume_name);

        info!("Adding container image metadata to volume");

        // Create XML with metadata
        let metadata_xml = format!(
            r#"<metadata>
  <bootc:container xmlns:bootc="https://github.com/containers/bootc">
    <bootc:source-image>{}</bootc:source-image>
    <bootc:filesystem>{}</bootc:filesystem>
    <bootc:created>{}</bootc:created>
    <bootc:bootc-kit-version>1.0.0</bootc:bootc-kit-version>
  </bootc:container>
</metadata>"#,
            self.source_image,
            self.install.filesystem.as_deref().unwrap_or("default"),
            chrono::Utc::now().to_rfc3339()
        );

        // Write metadata to temp file
        let temp_metadata = std::env::temp_dir().join("volume-metadata.xml");
        std::fs::write(&temp_metadata, metadata_xml)?;

        // Set the metadata on the volume
        let _output = Command::new("virsh")
            .args(&[
                "vol-desc",
                &volume_path,
                "--pool",
                &self.pool,
                "--edit",
                "--config",
            ])
            .output()?;

        // Alternative: Use vol-dumpxml, modify, and vol-create with XML
        // This is more reliable than vol-desc which might not support metadata

        // Get current volume XML
        let output = Command::new("virsh")
            .args(&["vol-dumpxml", &volume_path, "--pool", &self.pool])
            .output()?;

        if output.status.success() {
            let mut xml = String::from_utf8(output.stdout)?;

            // Insert metadata before closing </volume> tag
            let metadata = format!(
                r#"  <metadata>
    <bootc:container xmlns:bootc="https://github.com/containers/bootc">
      <bootc:source-image>{}</bootc:source-image>
      <bootc:filesystem>{}</bootc:filesystem>
      <bootc:created>{}</bootc:created>
      <bootc:bootc-kit-version>1.0.0</bootc:bootc-kit-version>
    </bootc:container>
  </metadata>
</volume>"#,
                self.source_image,
                self.install.filesystem.as_deref().unwrap_or("default"),
                chrono::Utc::now().to_rfc3339()
            );

            xml = xml.replace("</volume>", &metadata);

            // Save modified XML
            let temp_xml = std::env::temp_dir().join("volume-with-metadata.xml");
            std::fs::write(&temp_xml, xml)?;

            debug!("Added metadata to volume XML");
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_metadata);

        Ok(())
    }
}

/// Execute the libvirt disk upload process
pub fn run(opts: LibvirtUploadDiskOpts) -> Result<()> {
    info!(
        "Starting libvirt disk upload for image: {}",
        opts.source_image
    );

    // Phase 1: Calculate disk size to use
    let disk_size = if let Some(ref size_str) = opts.disk_size {
        // Use explicit size if provided
        utils::parse_size(size_str)?
    } else {
        // Use same logic as run_install: 2x source image size with 4GB minimum
        let image_size =
            images::get_image_size(&opts.source_image).unwrap_or(2 * 1024 * 1024 * 1024); // Default to 2GB if we can't get image size

        std::cmp::max(image_size * 2, 4 * 1024 * 1024 * 1024)
    };

    // Phase 2: Create temporary disk path
    let td = tempfile::TempDir::new()?;
    let td = td.path();
    let temp_disk = td.join("disk.img");
    info!("Using temporary disk: {temp_disk:?}");

    // Phase 3: Run installation to create disk image
    info!("Running bootc installation to create disk image");

    let install_opts = RunInstallOpts {
        source_image: opts.source_image.clone(),
        target_disk: temp_disk.clone(),
        install: opts.install.clone(),
        disk_size: Some(disk_size),
        common: crate::run_ephemeral::CommonVmOpts {
            memory: Some(opts.memory.clone()),
            vcpus: opts.vcpus,
            kernel_args: opts.karg.clone(),
            net: Some("none".to_string()),
            console: false,
            debug: false,
            virtio_serial_out: vec![],
            execute: Default::default(),
            ssh_keygen: false,
        },
    };

    run_install(install_opts)?;

    // Phase 4: Upload to libvirt (unless skipped)
    if !opts.skip_upload {
        opts.upload_to_libvirt(&temp_disk, disk_size)?;
        opts.add_volume_metadata()?;

        let volume_name = opts.get_volume_name();
        info!(
            "Successfully uploaded disk as volume '{}' to pool '{}'",
            volume_name, opts.pool
        );
        info!("Container image annotation added: {}", opts.source_image);
    }

    // Phase 5: Cleanup temporary disk (unless keep_temp is set)
    if !opts.keep_temp && !opts.skip_upload {
        info!("Cleaning up temporary disk");
        std::fs::remove_file(&temp_disk)?;
    } else if opts.keep_temp {
        info!("Keeping temporary disk at: {:?}", temp_disk);
    }

    Ok(())
}
