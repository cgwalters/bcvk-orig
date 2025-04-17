//! Domain listing utilities for podman-bootc VMs
//!
//! This module provides functionality to list libvirt domains created by bcvk pb,
//! using libvirt as the source of truth instead of the VmRegistry cache.

use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::SystemTime;

/// Information about a podman-bootc domain from libvirt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodmanBootcDomain {
    /// Domain name
    pub name: String,
    /// Domain state (running, shut off, etc.)
    pub state: String,
    /// Container image used to create the domain
    pub image: Option<String>,
    /// Domain creation timestamp (if available)
    pub created: Option<SystemTime>,
    /// Memory allocation in MB
    pub memory_mb: Option<u32>,
    /// Number of virtual CPUs  
    pub vcpus: Option<u32>,
    /// Disk path
    pub disk_path: Option<String>,
}

impl PodmanBootcDomain {
    /// Check if this domain is running
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }

    /// Check if this domain is stopped
    #[allow(dead_code)]
    pub fn is_stopped(&self) -> bool {
        self.state == "shut off"
    }

    /// Get status as string for display
    pub fn status_string(&self) -> String {
        match self.state.as_str() {
            "running" => "running".to_string(),
            "shut off" => "stopped".to_string(),
            "paused" => "paused".to_string(),
            other => other.to_string(),
        }
    }
}

/// Domain listing manager
pub struct DomainLister {
    /// Optional libvirt connection URI
    pub connect_uri: Option<String>,
}

impl Default for DomainLister {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainLister {
    /// Create a new domain lister
    pub fn new() -> Self {
        Self { connect_uri: None }
    }

    /// Create a domain lister with custom connection URI
    #[allow(dead_code)]
    pub fn with_connection(connect_uri: String) -> Self {
        Self {
            connect_uri: Some(connect_uri),
        }
    }

    /// Build a virsh command with optional connection URI
    fn virsh_command(&self) -> Command {
        let mut cmd = Command::new("virsh");
        if let Some(ref uri) = self.connect_uri {
            cmd.arg("-c").arg(uri);
        }
        cmd
    }

    /// List all domains (running and inactive)
    pub fn list_all_domains(&self) -> Result<Vec<String>> {
        let output = self
            .virsh_command()
            .args(&["list", "--all", "--name"])
            .output()
            .with_context(|| "Failed to run virsh list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(color_eyre::eyre::eyre!(
                "Failed to list domains: {}",
                stderr
            ));
        }

        let domain_names = String::from_utf8(output.stdout)?
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(domain_names)
    }

    /// Get domain state information
    pub fn get_domain_state(&self, domain_name: &str) -> Result<String> {
        let output = self
            .virsh_command()
            .args(&["domstate", domain_name])
            .output()
            .with_context(|| format!("Failed to get state for domain '{}'", domain_name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(color_eyre::eyre::eyre!(
                "Failed to get domain state for '{}': {}",
                domain_name,
                stderr
            ));
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Get domain XML metadata
    pub fn get_domain_xml(&self, domain_name: &str) -> Result<String> {
        let output = self
            .virsh_command()
            .args(&["dumpxml", domain_name])
            .output()
            .with_context(|| format!("Failed to dump XML for domain '{}'", domain_name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(color_eyre::eyre::eyre!(
                "Failed to get XML for domain '{}': {}",
                domain_name,
                stderr
            ));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    /// Extract podman-bootc metadata from domain XML
    fn extract_podman_bootc_metadata(&self, xml: &str) -> Option<PodmanBootcDomainMetadata> {
        // Look for bootc metadata in the XML
        // This could be in various forms:
        // 1. <bootc:source-image> in metadata section
        // 2. Domain name pattern (created by bcvk pb)
        // 3. Domain description containing bcvk signature

        // Try to extract source image from bootc metadata
        let source_image = extract_xml_value(xml, "bootc:source-image")
            .or_else(|| extract_xml_value(xml, "source-image"));

        // Extract other metadata
        let created =
            extract_xml_value(xml, "bootc:created").or_else(|| extract_xml_value(xml, "created"));

        // Extract memory and vcpu from domain XML
        let memory_mb = extract_xml_value(xml, "memory").and_then(|mem_str| {
            // Memory might have unit attribute, but we'll try to parse the value
            mem_str.parse::<u32>().ok()
        });

        let vcpus =
            extract_xml_value(xml, "vcpu").and_then(|vcpu_str| vcpu_str.parse::<u32>().ok());

        // Extract disk path from first disk device
        let disk_path = extract_disk_path(xml);

        Some(PodmanBootcDomainMetadata {
            source_image,
            created,
            memory_mb,
            vcpus,
            disk_path,
        })
    }

    /// Check if a domain was created by bcvk pb
    fn is_podman_bootc_domain(&self, _domain_name: &str, xml: &str) -> bool {
        // Only use XML metadata - domains created by bcvk pb should have bootc metadata
        xml.contains("bootc:source-image") || xml.contains("bootc:container")
    }

    /// Get detailed information about a domain
    pub fn get_domain_info(&self, domain_name: &str) -> Result<PodmanBootcDomain> {
        let state = self.get_domain_state(domain_name)?;
        let xml = self.get_domain_xml(domain_name)?;

        let metadata = self.extract_podman_bootc_metadata(&xml);

        Ok(PodmanBootcDomain {
            name: domain_name.to_string(),
            state,
            image: metadata.as_ref().and_then(|m| m.source_image.clone()),
            created: None, // TODO: Parse created timestamp
            memory_mb: metadata.as_ref().and_then(|m| m.memory_mb),
            vcpus: metadata.as_ref().and_then(|m| m.vcpus),
            disk_path: metadata.as_ref().and_then(|m| m.disk_path.clone()),
        })
    }

    /// List all bootc domains
    pub fn list_bootc_domains(&self) -> Result<Vec<PodmanBootcDomain>> {
        let all_domains = self.list_all_domains()?;
        let mut podman_bootc_domains = Vec::new();

        for domain_name in all_domains {
            // Get domain XML to check if it's a podman-bootc domain
            match self.get_domain_xml(&domain_name) {
                Ok(xml) => {
                    if self.is_podman_bootc_domain(&domain_name, &xml) {
                        match self.get_domain_info(&domain_name) {
                            Ok(domain_info) => podman_bootc_domains.push(domain_info),
                            Err(e) => {
                                eprintln!(
                                    "Warning: Failed to get info for domain '{}': {}",
                                    domain_name, e
                                );
                                // Continue with other domains
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to get XML for domain '{}': {}",
                        domain_name, e
                    );
                    // Continue with other domains
                }
            }
        }

        Ok(podman_bootc_domains)
    }

    /// List running bootc domains only
    pub fn list_running_bootc_domains(&self) -> Result<Vec<PodmanBootcDomain>> {
        let all_domains = self.list_bootc_domains()?;
        Ok(all_domains.into_iter().filter(|d| d.is_running()).collect())
    }
}

/// Internal structure for extracting metadata
#[derive(Debug)]
struct PodmanBootcDomainMetadata {
    source_image: Option<String>,
    #[allow(dead_code)]
    created: Option<String>,
    memory_mb: Option<u32>,
    vcpus: Option<u32>,
    disk_path: Option<String>,
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

    // Also try with attributes (e.g., <memory unit='MiB'>2048</memory>)
    let start_tag_with_attrs = format!("<{} ", element);
    if let Some(start_pos) = xml.find(&start_tag_with_attrs) {
        if let Some(close_pos) = xml[start_pos..].find('>') {
            let start = start_pos + close_pos + 1;
            if let Some(end_pos) = xml[start..].find(&end_tag) {
                let value = &xml[start..start + end_pos];
                return Some(value.trim().to_string());
            }
        }
    }

    None
}

/// Extract disk path from domain XML
fn extract_disk_path(xml: &str) -> Option<String> {
    // Look for first disk device with type="file"
    if let Some(disk_start) = xml.find("<disk type=\"file\"") {
        if let Some(source_start) = xml[disk_start..].find("<source file=\"") {
            let source_pos = disk_start + source_start + 14; // 14 = len("<source file=\"")
            if let Some(quote_end) = xml[source_pos..].find('"') {
                let path = &xml[source_pos..source_pos + quote_end];
                return Some(path.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_value() {
        let xml = r#"
        <domain>
            <memory unit='MiB'>2048</memory>
            <vcpu>4</vcpu>
            <metadata>
                <bootc:source-image>quay.io/fedora/fedora-bootc:40</bootc:source-image>
            </metadata>
        </domain>
        "#;

        assert_eq!(extract_xml_value(xml, "memory"), Some("2048".to_string()));
        assert_eq!(extract_xml_value(xml, "vcpu"), Some("4".to_string()));
        assert_eq!(
            extract_xml_value(xml, "bootc:source-image"),
            Some("quay.io/fedora/fedora-bootc:40".to_string())
        );
        assert_eq!(extract_xml_value(xml, "nonexistent"), None);
    }

    #[test]
    fn test_extract_disk_path() {
        let xml = r#"
        <domain>
            <devices>
                <disk type="file" device="disk">
                    <driver name="qemu" type="raw"/>
                    <source file="/var/lib/libvirt/images/test.raw"/>
                    <target dev="vda" bus="virtio"/>
                </disk>
            </devices>
        </domain>
        "#;

        assert_eq!(
            extract_disk_path(xml),
            Some("/var/lib/libvirt/images/test.raw".to_string())
        );
    }

    #[test]
    fn test_domain_status_mapping() {
        let domain = PodmanBootcDomain {
            name: "test".to_string(),
            state: "running".to_string(),
            image: None,
            created: None,
            memory_mb: None,
            vcpus: None,
            disk_path: None,
        };

        assert!(domain.is_running());
        assert!(!domain.is_stopped());
        assert_eq!(domain.status_string(), "running");

        let stopped_domain = PodmanBootcDomain {
            name: "test".to_string(),
            state: "shut off".to_string(),
            image: None,
            created: None,
            memory_mb: None,
            vcpus: None,
            disk_path: None,
        };

        assert!(!stopped_domain.is_running());
        assert!(stopped_domain.is_stopped());
        assert_eq!(stopped_domain.status_string(), "stopped");
    }
}
