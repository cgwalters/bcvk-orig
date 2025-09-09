//! Create and start libvirt domains from bootc volumes
//!
//! This module provides functionality to create libvirt domains using existing
//! bootc volumes in storage pools, with automatic resource configuration and
//! metadata-driven setup.

use crate::images;
use crate::install_options::InstallOptions;
use crate::libvirt::domain::DomainBuilder;
use crate::libvirt::upload::LibvirtUploadOpts;
use crate::run_ephemeral::{default_vcpus, DEFAULT_MEMORY_STR, DEFAULT_MEMORY_USER_STR};
use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use std::process::Command;
use tracing::{debug, info, warn};

/// Configuration options for creating a libvirt domain from bootc volume
#[derive(Debug, Parser)]
pub struct LibvirtCreateOpts {
    /// Name of the bootc volume to use for domain creation, OR container image to create from
    /// If this looks like a container image (contains '/' or ':'), will automatically upload if needed
    pub volume_name_or_image: String,

    /// Libvirt storage pool name
    #[clap(long, default_value = "default")]
    pub pool: String,

    /// Name for the libvirt domain (defaults to volume name)
    #[clap(long)]
    pub domain_name: Option<String>,

    /// Memory size for the domain (e.g. 2G, 1024M)
    #[clap(long, default_value = DEFAULT_MEMORY_USER_STR)]
    pub memory: String,

    /// Number of vCPUs for the domain
    #[clap(long, default_value_t = default_vcpus())]
    pub vcpus: u32,

    /// Network configuration (default, bridge=name, none)
    #[clap(long, default_value = "default")]
    pub network: String,

    /// Start the domain after creation
    #[clap(long)]
    pub start: bool,

    /// Enable VNC console access
    #[clap(long)]
    pub vnc: bool,

    /// Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)
    #[clap(short = 'c', long = "connect")]
    pub connect: Option<String>,

    /// VNC port (default: auto-assign)
    #[clap(long)]
    pub vnc_port: Option<u16>,

    /// Additional kernel arguments
    #[clap(long)]
    pub karg: Vec<String>,

    /// Dry run - validate configuration without creating domain
    #[clap(long)]
    pub dry_run: bool,

    /// Force creation even if domain already exists
    #[clap(long)]
    pub force: bool,

    /// Installation options for automatic upload (filesystem, root-size, storage-path)
    #[clap(flatten)]
    pub install: InstallOptions,

    /// Size of the disk image for automatic upload (e.g., '20G', '10240M')
    #[clap(long)]
    pub disk_size: Option<String>,

    /// Memory size for installation VM during auto-upload (e.g. 2G, 1024M)
    #[clap(long, default_value = DEFAULT_MEMORY_STR)]
    pub install_memory: String,

    /// Number of vCPUs for installation VM during auto-upload
    #[clap(long, default_value_t = default_vcpus())]
    pub install_vcpus: u32,
}

/// Metadata extracted from bootc volume
#[derive(Debug)]
pub struct BootcVolumeMetadata {
    pub source_image: Option<String>,
}

impl LibvirtCreateOpts {
    /// Build a virsh command with optional connection URI
    fn virsh_command(&self) -> Command {
        let mut cmd = Command::new("virsh");
        if let Some(ref connect) = self.connect {
            cmd.arg("-c").arg(connect);
        }
        cmd
    }

    /// Check if the input appears to be a container image (vs volume name)
    fn is_container_image(&self) -> bool {
        // Container images typically contain '/' or ':' characters
        // Volume names are typically simple strings without these
        self.volume_name_or_image.contains('/')
            || (self.volume_name_or_image.contains(':')
                && !self.volume_name_or_image.ends_with(".raw"))
    }

    /// Generate domain name from volume name or container image if not specified
    fn get_domain_name(&self) -> String {
        self.domain_name.clone().unwrap_or_else(|| {
            if self.is_container_image() {
                // For container images, create a sanitized name
                let image_name = self.volume_name_or_image.clone();
                let name = image_name
                    .split('/')
                    .last()
                    .unwrap_or(&image_name)
                    .replace(':', "-")
                    .replace('/', "-")
                    .replace('.', "-");
                format!("bootc-{}", name)
            } else {
                format!("bootc-{}", self.volume_name_or_image)
            }
        })
    }

    /// Find existing volume by container image digest using name-based lookup
    fn find_cached_volume(&self, image_digest: &str) -> Result<Option<String>> {
        info!("Looking for cached volume with digest: {}", image_digest);

        // Create a temporary upload opts to get the expected cached volume name
        let temp_upload_opts = LibvirtUploadOpts {
            source_image: self.volume_name_or_image.clone(),
            volume_name: None,
            pool: self.pool.clone(),
            disk_size: None,
            install: self.install.clone(),
            memory: DEFAULT_MEMORY_STR.to_string(),
            vcpus: default_vcpus(),
            karg: vec![],
            connect: self.connect.clone(),
            skip_upload: true,
            keep_temp: false,
        };

        let expected_volume_name = temp_upload_opts.get_cached_volume_name(image_digest);
        let expected_volume_path = format!("{}.raw", expected_volume_name);

        // Check if this specific volume exists
        let output = self
            .virsh_command()
            .args(&["vol-info", &expected_volume_path, "--pool", &self.pool])
            .output()?;

        if output.status.success() {
            info!("Found cached volume: {}", expected_volume_name);
            return Ok(Some(expected_volume_name));
        }

        info!("No cached volume found for digest: {}", image_digest);
        Ok(None)
    }

    /// Automatically upload container image if no cached volume exists
    fn ensure_volume_exists(&self) -> Result<String> {
        if !self.is_container_image() {
            // If it's already a volume name, just return it
            return Ok(self.volume_name_or_image.clone());
        }

        // It's a container image, check for cached volume
        let image_digest = images::get_image_digest(&self.volume_name_or_image)?;

        if let Some(cached_volume) = self.find_cached_volume(&image_digest)? {
            info!("Using cached volume: {}", cached_volume);
            return Ok(cached_volume);
        }

        // No cached volume found, need to upload
        info!(
            "No cached volume found, uploading container image: {}",
            self.volume_name_or_image
        );

        let upload_opts = LibvirtUploadOpts {
            source_image: self.volume_name_or_image.clone(),
            volume_name: None, // Let it auto-generate
            pool: self.pool.clone(),
            disk_size: self.disk_size.clone(),
            install: self.install.clone(),
            memory: self.install_memory.clone(),
            vcpus: self.install_vcpus,
            karg: self.karg.clone(),
            connect: self.connect.clone(),
            skip_upload: false,
            keep_temp: false,
        };

        // Run the upload
        crate::libvirt::upload::run(upload_opts.clone())?;

        // Return the generated volume name (with digest)
        Ok(upload_opts.get_cached_volume_name(&image_digest))
    }

    /// Parse memory size to MB
    fn parse_memory(&self) -> Result<u64> {
        crate::utils::parse_size(&self.memory).map(|bytes| bytes / (1024 * 1024))
    }

    /// Check if volume exists in the specified pool
    fn check_volume_exists(&self, volume_name: &str) -> Result<String> {
        let volume_path = if volume_name.ends_with(".raw") {
            volume_name.to_string()
        } else {
            format!("{}.raw", volume_name)
        };

        let output = self
            .virsh_command()
            .args(&["vol-info", &volume_path, "--pool", &self.pool])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Volume '{}' not found in pool '{}': {}",
                volume_path,
                self.pool,
                stderr
            ));
        }

        // Get the full volume path
        let vol_path_output = self
            .virsh_command()
            .args(&["vol-path", &volume_path, "--pool", &self.pool])
            .output()?;

        if vol_path_output.status.success() {
            let path = String::from_utf8(vol_path_output.stdout)?;
            Ok(path.trim().to_string())
        } else {
            Err(eyre!("Failed to get volume path for '{}'", volume_path))
        }
    }

    /// Extract metadata from bootc volume
    fn get_volume_metadata(&self, volume_name: &str) -> Result<BootcVolumeMetadata> {
        let volume_path = if volume_name.ends_with(".raw") {
            volume_name.to_string()
        } else {
            format!("{}.raw", volume_name)
        };

        let output = self
            .virsh_command()
            .args(&["vol-dumpxml", &volume_path, "--pool", &self.pool])
            .output()?;
        if !output.status.success() {
            return Err(eyre!("Failed to dumpxml: {:?}", output.status));
        }

        let xml = String::from_utf8(output.stdout)?;
        debug!("Volume XML: {}", xml);

        // Parse XML to extract bootc metadata
        // For simplicity, using string parsing - could use proper XML parser
        let source_image = extract_xml_value(&xml, "bootc:source-image");
        Ok(BootcVolumeMetadata { source_image })
    }

    /// Check if domain already exists
    fn check_domain_exists(&self, domain_name: &str) -> bool {
        let output = self
            .virsh_command()
            .args(&["dominfo", domain_name])
            .output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Create the libvirt domain
    fn create_domain(
        &self,
        volume_path: &str,
        metadata: &BootcVolumeMetadata,
        volume_name: &str,
    ) -> Result<()> {
        let domain_name = self.get_domain_name();
        let memory_mb = self.parse_memory()?;

        info!(
            "Creating domain '{}' from volume '{}' in pool '{}'",
            domain_name, volume_name, self.pool
        );

        if self.check_domain_exists(&domain_name) && !self.force {
            return Err(eyre!(
                "Domain '{}' already exists. Use --force to recreate.",
                domain_name
            ));
        }

        // If domain exists and force is specified, undefine it first
        if self.check_domain_exists(&domain_name) && self.force {
            info!("Domain exists, removing it first (--force specified)");
            let _ = self
                .virsh_command()
                .args(&["destroy", &domain_name])
                .output();
            let _ = self
                .virsh_command()
                .args(&["undefine", &domain_name])
                .output();
        }

        if self.dry_run {
            info!("Dry run mode - would create domain with:");
            info!("  Name: {}", domain_name);
            info!("  Memory: {} MB", memory_mb);
            info!("  vCPUs: {}", self.vcpus);
            info!("  Volume: {}", volume_path);
            info!("  Network: {}", self.network);
            if let Some(ref source_image) = metadata.source_image {
                info!("  Source Image: {}", source_image);
            }
            return Ok(());
        }

        // Build domain configuration
        let mut domain_builder = DomainBuilder::new()
            .with_name(&domain_name)
            .with_memory(memory_mb)
            .with_vcpus(self.vcpus)
            .with_disk(volume_path)
            .with_network(&self.network);

        if self.vnc {
            let port = self.vnc_port.unwrap_or(5900 + self.vcpus as u16);
            domain_builder = domain_builder.with_vnc(port);
        }

        if !self.karg.is_empty() {
            domain_builder = domain_builder.with_kernel_args(&self.karg.join(" "));
        }

        // Add metadata to domain
        if let Some(ref source_image) = metadata.source_image {
            domain_builder = domain_builder.with_metadata("bootc:source-image", source_image);
        }

        let domain_xml = domain_builder.build_xml()?;

        debug!("Domain XML: {}", domain_xml);

        // Define the domain
        let output = self
            .virsh_command()
            .args(&["define", "/dev/stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut child = output;
        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin.write_all(domain_xml.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to define domain: {}", stderr));
        }

        info!("Domain '{}' created successfully", domain_name);

        // Start domain if requested
        if self.start {
            info!("Starting domain '{}'", domain_name);
            let output = self
                .virsh_command()
                .args(&["start", &domain_name])
                .output()?;

            if output.status.success() {
                info!("Domain '{}' started successfully", domain_name);

                // Show connection information
                self.show_connection_info(&domain_name)?;
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to start domain: {}", stderr);
            }
        }

        Ok(())
    }

    /// Display connection information for the created domain
    fn show_connection_info(&self, domain_name: &str) -> Result<()> {
        info!("Domain '{}' connection information:", domain_name);

        if self.vnc {
            let port = self.vnc_port.unwrap_or(5900 + self.vcpus as u16);
            info!("  VNC Console: vnc://localhost:{}", port);
        }

        info!("  Serial Console: virsh console {}", domain_name);
        info!("  SSH Access: bootc-kit libvirt ssh {}", domain_name);

        Ok(())
    }
}

/// Extract value from XML element (simple string parsing)
fn extract_xml_value(xml: &str, element: &str) -> Option<String> {
    let start_tag = format!("<{}>", element);
    let end_tag = format!("</{}>", element);

    if let Some(start_pos) = xml.find(&start_tag) {
        let start = start_pos + start_tag.len();
        if let Some(end_pos) = xml[start..].find(&end_tag) {
            let value = &xml[start..start + end_pos];
            return Some(value.trim().to_string());
        }
    }
    None
}

/// Execute the libvirt domain creation process
pub fn run(opts: LibvirtCreateOpts) -> Result<()> {
    info!(
        "Creating libvirt domain from: {}",
        opts.volume_name_or_image
    );

    // Phase 1: Ensure volume exists (auto-upload if needed)
    let volume_name = opts.ensure_volume_exists()?;

    // Phase 2: Validate volume exists and get path
    let volume_path = opts.check_volume_exists(&volume_name)?;
    info!("Found volume at: {}", volume_path);

    // Phase 3: Extract volume metadata
    let metadata = opts.get_volume_metadata(&volume_name)?;
    if let Some(ref source_image) = metadata.source_image {
        info!("Volume contains bootc image: {}", source_image);
    }

    // Phase 4: Create and optionally start domain
    opts.create_domain(&volume_path, &metadata, &volume_name)?;

    Ok(())
}
