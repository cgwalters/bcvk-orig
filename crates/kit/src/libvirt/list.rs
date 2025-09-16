//! List bootc volumes in libvirt storage pools
//!
//! This module provides functionality to discover and list bootc volumes
//! with their container image metadata and creation information.

use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use comfy_table::{presets::UTF8_FULL, Table};
use serde_json::{json, Value};
use std::process::Command;
use tracing::{debug, warn};

/// Configuration options for listing bootc volumes
#[derive(Debug, Parser)]
pub struct LibvirtListOpts {
    /// Libvirt storage pool name to search
    #[clap(long, default_value = "default")]
    pub pool: String,

    /// Output format (human-readable or JSON)
    #[clap(long)]
    pub json: bool,

    /// Show detailed volume information
    #[clap(long)]
    pub detailed: bool,

    /// Filter by source container image
    #[clap(long)]
    pub source_image: Option<String>,

    /// Show all volumes (not just bootc volumes)
    #[clap(long)]
    pub all: bool,

    /// Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)
    #[clap(short = 'c', long = "connect")]
    pub connect: Option<String>,
}

/// Information about a bootc volume
#[derive(Debug, PartialEq)]
pub struct BootcVolume {
    pub name: String,
    pub size: u64,
    pub format: String,
    pub path: String,
    pub source_image: Option<String>,
    pub source_digest: Option<String>,
    pub created: Option<String>,
}

impl BootcVolume {
    /// Check if this volume appears to be a bootc volume
    fn is_bootc_volume(&self) -> bool {
        // A volume is considered a bootc volume if:
        // 1. It has bootc metadata (source_image)
        // 2. Its name starts with "bootc-"
        self.source_image.is_some() || self.name.starts_with("bootc-")
    }

    /// Convert to JSON representation
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "size": self.size,
            "format": self.format,
            "path": self.path,
            "source_image": self.source_image,
            "source_digest": self.source_digest,
            "created": self.created,
        })
    }
}

impl LibvirtListOpts {
    /// Build a virsh command with optional connection URI
    fn virsh_command(&self) -> Command {
        let mut cmd = Command::new("virsh");
        if let Some(ref connect) = self.connect {
            cmd.arg("-c").arg(connect);
        }
        cmd
    }

    /// Check if storage pool exists and is accessible
    fn check_pool_exists(&self) -> Result<()> {
        let output = self
            .virsh_command()
            .args(&["pool-info", &self.pool])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Cannot access storage pool '{}': {}",
                self.pool,
                stderr
            ));
        }

        Ok(())
    }

    /// List all volumes in the storage pool
    pub fn list_pool_volumes(&self) -> Result<Vec<String>> {
        let output = self
            .virsh_command()
            .args(&["vol-list", &self.pool])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Failed to list volumes in pool '{}': {}",
                self.pool,
                stderr
            ));
        }

        let volume_names = String::from_utf8(output.stdout)?
            .lines()
            .skip(2) // Skip header lines
            .map(|line| {
                // Extract volume name from table format
                // Format is usually: " volume-name.raw       /path/to/volume"
                line.trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .filter(|line| !line.is_empty() && !line.starts_with('-'))
            .collect();

        Ok(volume_names)
    }

    /// Get volume information including metadata
    pub fn get_volume_info(&self, volume_name: &str) -> Result<BootcVolume> {
        // Get volume path
        let path_output = self
            .virsh_command()
            .args(&["vol-path", volume_name, "--pool", &self.pool])
            .output()?;

        let path = if path_output.status.success() {
            String::from_utf8(path_output.stdout)?.trim().to_string()
        } else {
            format!("(unknown path)")
        };

        // Get volume info (size, format)
        let info_output = self
            .virsh_command()
            .args(&["vol-info", volume_name, "--pool", &self.pool])
            .output()?;

        let mut size = 0u64;
        let mut format = "unknown".to_string();

        if info_output.status.success() {
            let info = String::from_utf8(info_output.stdout)?;

            // Parse volume info
            for line in info.lines() {
                if line.starts_with("Capacity:") {
                    if let Some(size_str) = line.split_whitespace().nth(1) {
                        size = parse_virsh_size(size_str).unwrap_or(0);
                    }
                } else if line.starts_with("Type:") {
                    if let Some(format_str) = line.split_whitespace().nth(1) {
                        format = format_str.to_string();
                    }
                }
            }
        }

        // Get metadata from volume XML
        let xml_output = self
            .virsh_command()
            .args(&["vol-dumpxml", volume_name, "--pool", &self.pool])
            .output()?;

        let mut source_image = None;
        let mut source_digest = None;
        let mut created = None;

        if xml_output.status.success() {
            let xml = String::from_utf8(xml_output.stdout)?;
            debug!("Volume XML for {}: {}", volume_name, xml);

            // First try to extract metadata from description field (new format)
            if let Some(description) = extract_xml_value(&xml, "description") {
                if description.starts_with("bcvk volume: ") {
                    // Parse JSON metadata from description
                    let json_str = description.strip_prefix("bcvk volume: ").unwrap_or("");
                    if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(json_str) {
                        source_image = metadata
                            .get("source_image")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        source_digest = metadata
                            .get("source_digest")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        created = metadata
                            .get("created")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }

            // Fallback to old metadata format (bootc: namespace)
            if source_image.is_none() {
                source_image = extract_xml_value(&xml, "bootc:source-image");
                source_digest = extract_xml_value(&xml, "bootc:source-digest");
                created = extract_xml_value(&xml, "bootc:created");
            }
        }

        Ok(BootcVolume {
            name: volume_name.to_string(),
            size,
            format,
            path,
            source_image,
            source_digest,
            created,
        })
    }

    /// Filter volumes based on options
    fn filter_volumes(&self, volumes: Vec<BootcVolume>) -> Vec<BootcVolume> {
        volumes
            .into_iter()
            .filter(|vol| {
                // Filter by bootc volumes unless --all specified
                if !self.all && !vol.is_bootc_volume() {
                    return false;
                }

                // Filter by source image if specified
                if let Some(ref filter_image) = self.source_image {
                    if let Some(ref vol_image) = vol.source_image {
                        return vol_image.contains(filter_image);
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Display volumes in human-readable format
    fn display_human(&self, volumes: &[BootcVolume]) -> Result<()> {
        if volumes.is_empty() {
            if self.all {
                println!("No volumes found in pool '{}'", self.pool);
            } else {
                println!("No bootc volumes found in pool '{}'", self.pool);
                println!("Use --all to see all volumes");
            }
            return Ok(());
        }

        // Create table using comfy_table
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        if self.detailed {
            table.set_header(vec![
                "NAME",
                "SIZE",
                "FORMAT",
                "PATH",
                "SOURCE IMAGE",
                "CREATED",
            ]);

            for volume in volumes {
                let source_image = volume.source_image.as_deref().unwrap_or("<no metadata>");
                let created = volume.created.as_deref().unwrap_or("N/A");

                table.add_row(vec![
                    &volume.name,
                    &format_size(volume.size),
                    &volume.format,
                    &volume.path,
                    source_image,
                    created,
                ]);
            }
        } else {
            table.set_header(vec!["NAME", "SIZE", "SOURCE IMAGE"]);

            for volume in volumes {
                let source_image = volume.source_image.as_deref().unwrap_or("<no metadata>");

                table.add_row(vec![&volume.name, &format_size(volume.size), source_image]);
            }
        }

        println!("{}", table);

        // Summary
        println!(
            "\nFound {} volume{} in pool '{}'",
            volumes.len(),
            if volumes.len() == 1 { "" } else { "s" },
            self.pool
        );

        Ok(())
    }

    /// Display volumes in JSON format
    fn display_json(&self, volumes: &[BootcVolume]) -> Result<()> {
        let json_volumes: Vec<Value> = volumes.iter().map(|v| v.to_json()).collect();

        let output = json!({
            "pool": self.pool,
            "volume_count": volumes.len(),
            "volumes": json_volumes
        });

        println!("{}", serde_json::to_string_pretty(&output)?);
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

/// Parse virsh size format (e.g., "5.00 GiB") to bytes
fn parse_virsh_size(size_str: &str) -> Option<u64> {
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let number: f64 = parts[0].parse().ok()?;
    let unit = parts[1];

    let multiplier = match unit {
        "B" | "bytes" => 1,
        "KiB" | "KB" => 1024,
        "MiB" | "MB" => 1024 * 1024,
        "GiB" | "GB" => 1024 * 1024 * 1024,
        "TiB" | "TB" => 1024u64.pow(4),
        _ => return None,
    };

    Some((number * multiplier as f64) as u64)
}

/// Format size in bytes to human-readable format
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes < THRESHOLD {
        return format!("{}B", bytes);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    format!("{:.1}{}", size, UNITS[unit_index])
}

/// Execute the libvirt volume listing process
pub fn run(opts: LibvirtListOpts) -> Result<()> {
    debug!("Listing volumes in libvirt pool: {}", opts.pool);

    // Phase 1: Check pool exists
    opts.check_pool_exists()?;

    // Phase 2: List all volumes in pool
    let volume_names = opts.list_pool_volumes()?;

    if volume_names.is_empty() {
        if opts.json {
            let output = json!({
                "pool": opts.pool,
                "volume_count": 0,
                "volumes": []
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("No volumes found in pool '{}'", opts.pool);
        }
        return Ok(());
    }

    // Phase 3: Get detailed info for each volume
    let mut volumes = Vec::new();
    for volume_name in volume_names {
        match opts.get_volume_info(&volume_name) {
            Ok(volume_info) => volumes.push(volume_info),
            Err(e) => {
                warn!("Failed to get info for volume '{}': {}", volume_name, e);
                // Continue with other volumes
            }
        }
    }

    // Phase 4: Filter volumes based on criteria
    let filtered_volumes = opts.filter_volumes(volumes);

    // Phase 5: Display results
    if opts.json {
        opts.display_json(&filtered_volumes)?;
    } else {
        opts.display_human(&filtered_volumes)?;
    }

    Ok(())
}
