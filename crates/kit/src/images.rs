//! Image management and inspection utilities for bootc containers.
//!
//! Provides functionality for listing and inspecting bootc container images through
//! podman integration with both table and JSON output formats.

use std::collections::HashMap;

use bootc_utils::CommandRunExt;
use color_eyre::{eyre::eyre, Result};
use comfy_table::{presets::UTF8_FULL, Table};
use serde::{Deserialize, Serialize};

use crate::hostexec;

/// Command-line options for image management operations.
#[derive(clap::Subcommand, Debug)]
pub(crate) enum ImagesOpts {
    /// List all available bootc container images on the system
    List {
        /// Output as structured JSON instead of table format
        #[clap(long)]
        json: bool,
    },
}

impl ImagesOpts {
    pub(crate) fn run(self) -> Result<()> {
        match self {
            ImagesOpts::List { json } => {
                let images = list()?;

                if json {
                    let json_output = serde_json::to_string_pretty(&images)?;
                    println!("{}", json_output);
                } else {
                    // Create a table using comfy_table
                    let mut table = Table::new();
                    table.load_preset(UTF8_FULL).set_header(vec![
                        "REPOSITORY",
                        "TAG",
                        "IMAGE ID",
                        "CREATED",
                        "SIZE",
                    ]);

                    for image in images {
                        let (repository, tag) = if let Some(names) = &image.names {
                            if let Some(name) = names.first() {
                                if let Some((repo, tag)) = name.rsplit_once(':') {
                                    (repo.to_string(), tag.to_string())
                                } else {
                                    (name.to_string(), "latest".to_string())
                                }
                            } else {
                                ("<none>".to_string(), "<none>".to_string())
                            }
                        } else {
                            ("<none>".to_string(), "<none>".to_string())
                        };

                        let id = if image.id.len() > 12 {
                            &image.id[..12]
                        } else {
                            &image.id
                        };

                        let created = image
                            .created_at
                            .map(|dt| format_relative_time(dt))
                            .unwrap_or_else(|| "N/A".to_string());

                        let size = format_size(image.size);

                        table.add_row(vec![repository, tag, id.to_string(), created, size]);
                    }

                    println!("{}", table);
                }
                Ok(())
            }
        }
    }
}

/// Single bootc container image entry from podman images output.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImageListEntry {
    /// Repository names and tags, None for dangling images
    pub names: Option<Vec<String>>,

    /// SHA256 image identifier
    pub id: String,

    /// Image size in bytes
    pub size: u64,

    /// Image creation timestamp
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Container image inspection data from podman image inspect.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImageInspect {
    /// SHA256 image identifier
    pub id: String,

    /// Image size in bytes
    pub size: u64,

    /// Image creation timestamp
    pub created: Option<chrono::DateTime<chrono::Utc>>,
}

/// Format bytes into human-readable size string.
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

/// Format a datetime as relative time (e.g., "2 hours ago", "3 days ago").
fn format_relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        format!("{} seconds ago", duration.num_seconds())
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", mins)
        }
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else if duration.num_days() < 30 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else {
        let years = duration.num_days() / 365;
        if years == 1 {
            "1 year ago".to_string()
        } else {
            format!("{} years ago", years)
        }
    }
}

/// Parse os-release file format into key-value pairs.
#[allow(dead_code)]
fn parse_osrelease(s: &str) -> Result<HashMap<String, String>> {
    let r = s
        .lines()
        .filter_map(|line| {
            let Some((k, v)) = line.split_once('=') else {
                return None;
            };
            if k.starts_with('#') {
                return None;
            }
            let Some(v) = shlex::split(v) else {
                return None;
            };
            let Some(v) = v.into_iter().next() else {
                return None;
            };
            Some((k.to_string(), v.to_string()))
        })
        .collect();
    Ok(r)
}

/// List all bootc container images using podman.
#[allow(dead_code)]
pub fn list() -> Result<Vec<ImageListEntry>> {
    let images: Vec<ImageListEntry> = hostexec::command("podman", None)?
        .args([
            "images",
            "--format",
            "json",
            "--filter=label=containers.bootc=1",
        ])
        .run_and_parse_json()
        .map_err(|e| eyre!("{e}"))?;
    Ok(images)
}

/// Inspect a container image and return metadata.
pub fn inspect(name: &str) -> Result<ImageInspect> {
    let mut r: Vec<ImageInspect> = hostexec::command("podman", None)?
        .args(["image", "inspect", name])
        .run_and_parse_json()
        .map_err(|e| eyre!("{e}"))?;
    r.pop().ok_or_else(|| eyre!("No such image"))
}

/// Get container image size in bytes for disk space planning.
pub fn get_image_size(name: &str) -> Result<u64> {
    tracing::debug!("Getting size for image: {}", name);
    let info = inspect(name)?;
    tracing::debug!("Found image size: {} bytes", info.size);
    Ok(info.size)
}

/// Get container image digest (sha256) for caching purposes.
/// Returns the digest in the format "sha256:abc123..."
pub fn get_image_digest(name: &str) -> Result<String> {
    tracing::debug!("Getting digest for image: {}", name);

    // Use skopeo inspect to get the manifest digest, which is more reliable than podman
    // for getting the actual @sha256 digest that can be used for caching
    let output = hostexec::command("skopeo", None)?
        .args(["inspect", &format!("containers-storage:{}", name)])
        .run_and_parse_json::<serde_json::Value>()
        .map_err(|e| eyre!("Failed to inspect image with skopeo: {}", e))?;

    // Extract the digest from the skopeo output
    if let Some(digest) = output.get("Digest").and_then(|d| d.as_str()) {
        tracing::debug!("Found image digest: {}", digest);
        Ok(digest.to_string())
    } else {
        // Fall back to podman image inspect
        tracing::debug!("No digest in skopeo output, falling back to podman inspect");
        let info = inspect(name)?;
        // Podman ID is already in sha256:xxx format
        if info.id.starts_with("sha256:") {
            Ok(info.id)
        } else {
            Ok(format!("sha256:{}", info.id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osrelease() {
        let input = r#"NAME="Fedora Linux"
VERSION="39 (Container Image)"
ID=fedora
VERSION_ID=39
PLATFORM_ID="platform:f39"
PRETTY_NAME="Fedora Linux 39 (Container Image)"
# Comment here then a blank line

LOGO="fedora-logo-icon"
# Trailing comment
"#;
        let expected = [
            ("NAME", "Fedora Linux"),
            ("VERSION", "39 (Container Image)"),
            ("ID", "fedora"),
            ("VERSION_ID", "39"),
            ("PLATFORM_ID", "platform:f39"),
            ("PRETTY_NAME", "Fedora Linux 39 (Container Image)"),
            ("LOGO", "fedora-logo-icon"),
        ];
        let actual = parse_osrelease(input).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (k, v) in expected {
            assert_eq!(actual.get(k).unwrap(), v);
        }
    }

    #[test]
    fn test_disk_size_calculation_logic() {
        // Test the logic used in calculate_disk_size
        let image_size: u64 = 1024 * 1024 * 1024; // 1GB
        let expected_size = image_size * 2; // 2GB
        let minimum_size = 4 * 1024 * 1024 * 1024; // 4GB

        // Since 2GB < 4GB minimum, should use 4GB
        let final_size = std::cmp::max(expected_size, minimum_size);
        assert_eq!(final_size, minimum_size);

        // Test with larger image
        let large_image_size: u64 = 3 * 1024 * 1024 * 1024; // 3GB
        let large_expected = large_image_size * 2; // 6GB
        let large_final = std::cmp::max(large_expected, minimum_size);
        assert_eq!(large_final, large_expected); // Should use 6GB, not minimum
    }
}
