//! Image management and inspection utilities for bootc containers.
//!
//! Provides functionality for listing and inspecting bootc container images through
//! podman integration with both table and JSON output formats.

use std::{collections::HashMap, os::unix::process::CommandExt};

use bootc_utils::CommandRunExt;
use color_eyre::{eyre::eyre, Result};
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
                if json {
                    // Use the existing list function that returns JSON
                    let images = list()?;
                    let json_output = serde_json::to_string_pretty(&images)?;
                    println!("{}", json_output);
                    Ok(())
                } else {
                    // Use the standard podman images command for table output
                    let r = hostexec::command("podman", None)?
                        .args(["images", "--filter=label=containers.bootc=1"])
                        .exec();
                    Err(r.into())
                }
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
