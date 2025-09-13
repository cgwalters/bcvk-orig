//! VM registry for tracking podman-bootc VMs
//!
//! Manages persistent VM metadata, state tracking, and disk image location.

use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// VM status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VmStatus {
    /// VM has been created but not started
    Created,
    /// VM is currently running
    Running,
    /// VM is stopped
    Stopped,
    /// VM creation failed or is in an error state
    Error(String),
}

/// VM metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmMetadata {
    /// Unique VM name
    pub name: String,
    /// Container image used to create the VM
    pub image: String,
    /// VM creation timestamp
    pub created: SystemTime,
    /// Last modified timestamp
    pub modified: SystemTime,
    /// Current VM status
    pub status: VmStatus,
    /// Libvirt domain name (if created)
    pub libvirt_domain: Option<String>,
    /// SSH port for accessing the VM
    pub ssh_port: Option<u16>,
    /// Memory allocation in MB
    pub memory_mb: u32,
    /// Number of virtual CPUs
    pub vcpus: u32,
    /// Disk image path
    pub disk_path: PathBuf,
    /// Disk size in GB
    pub disk_size_gb: u32,
    /// Root filesystem type
    pub filesystem: String,
    /// Network mode
    pub network: String,
    /// Port mappings (host:guest)
    pub port_mappings: Vec<String>,
    /// Volume mappings (host:guest[:options])
    pub volumes: Vec<String>,
}

impl VmMetadata {
    /// Create new VM metadata
    pub fn new(
        name: String,
        image: String,
        memory_mb: u32,
        vcpus: u32,
        disk_size_gb: u32,
        filesystem: String,
        network: String,
        port_mappings: Vec<String>,
        volumes: Vec<String>,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            name: name.clone(),
            image,
            created: now,
            modified: now,
            status: VmStatus::Created,
            libvirt_domain: None,
            ssh_port: None,
            memory_mb,
            vcpus,
            disk_path: PathBuf::new(), // Will be set when disk is created
            disk_size_gb,
            filesystem,
            network,
            port_mappings,
            volumes,
        }
    }

    /// Update VM status
    pub fn set_status(&mut self, status: VmStatus) {
        self.status = status;
        self.modified = SystemTime::now();
    }

    /// Set libvirt domain name
    pub fn set_libvirt_domain(&mut self, domain: String) {
        self.libvirt_domain = Some(domain);
        self.modified = SystemTime::now();
    }

    /// Set SSH port
    pub fn set_ssh_port(&mut self, port: u16) {
        self.ssh_port = Some(port);
        self.modified = SystemTime::now();
    }

    /// Set disk path
    pub fn set_disk_path(&mut self, path: PathBuf) {
        self.disk_path = path;
        self.modified = SystemTime::now();
    }

    /// Check if VM is running
    pub fn is_running(&self) -> bool {
        matches!(self.status, VmStatus::Running)
    }

    /// Check if VM is stopped
    pub fn is_stopped(&self) -> bool {
        matches!(self.status, VmStatus::Stopped)
    }

    /// Check if VM has errors
    pub fn has_error(&self) -> bool {
        matches!(self.status, VmStatus::Error(_))
    }

    /// Get status as string
    pub fn status_string(&self) -> String {
        match &self.status {
            VmStatus::Created => "created".to_string(),
            VmStatus::Running => "running".to_string(),
            VmStatus::Stopped => "stopped".to_string(),
            VmStatus::Error(msg) => format!("error: {}", msg),
        }
    }
}

/// VM registry for managing multiple VMs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmRegistry {
    /// Map of VM name to metadata
    pub vms: HashMap<String, VmMetadata>,
    /// Registry version for future compatibility
    pub version: u32,
    /// Last update timestamp
    pub last_updated: SystemTime,
}

impl Default for VmRegistry {
    fn default() -> Self {
        Self {
            vms: HashMap::new(),
            version: 1,
            last_updated: SystemTime::now(),
        }
    }
}

impl VmRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a VM to the registry
    pub fn add_vm(&mut self, vm: VmMetadata) -> Result<()> {
        if self.vms.contains_key(&vm.name) {
            return Err(color_eyre::eyre::eyre!("VM '{}' already exists", vm.name));
        }

        self.vms.insert(vm.name.clone(), vm);
        self.last_updated = SystemTime::now();
        Ok(())
    }

    /// Get a VM by name
    pub fn get_vm(&self, name: &str) -> Option<&VmMetadata> {
        self.vms.get(name)
    }

    /// Get a mutable VM by name
    pub fn get_vm_mut(&mut self, name: &str) -> Option<&mut VmMetadata> {
        if self.vms.contains_key(name) {
            self.last_updated = SystemTime::now();
        }
        self.vms.get_mut(name)
    }

    /// Remove a VM from the registry
    pub fn remove_vm(&mut self, name: &str) -> Option<VmMetadata> {
        self.last_updated = SystemTime::now();
        self.vms.remove(name)
    }

    /// List all VMs
    pub fn list_vms(&self) -> Vec<&VmMetadata> {
        self.vms.values().collect()
    }

    /// List all VM names
    pub fn list_vm_names(&self) -> Vec<&String> {
        self.vms.keys().collect()
    }

    /// Get running VMs
    pub fn get_running_vms(&self) -> Vec<&VmMetadata> {
        self.vms.values().filter(|vm| vm.is_running()).collect()
    }

    /// Get the most recently created VM
    pub fn get_latest_vm(&self) -> Option<&VmMetadata> {
        self.vms.values().max_by_key(|vm| vm.created)
    }

    /// Update VM status
    pub fn update_vm_status(&mut self, name: &str, status: VmStatus) -> Result<()> {
        match self.get_vm_mut(name) {
            Some(vm) => {
                vm.set_status(status);
                Ok(())
            }
            None => Err(color_eyre::eyre::eyre!("VM '{}' not found", name)),
        }
    }

    /// Check if a VM name is available
    pub fn is_name_available(&self, name: &str) -> bool {
        !self.vms.contains_key(name)
    }

    /// Generate a unique VM name from image
    pub fn generate_vm_name(&self, image: &str) -> String {
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

        while !self.is_name_available(&candidate) {
            counter += 1;
            candidate = format!("{}-{}", sanitized, counter);
        }

        candidate
    }

    /// Get total number of VMs
    pub fn count(&self) -> usize {
        self.vms.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.vms.is_empty()
    }
}

/// VM registry manager for persistent storage
pub struct VmRegistryManager {
    registry_path: PathBuf,
    data_dir: PathBuf,
}

impl VmRegistryManager {
    /// Create a new registry manager
    pub fn new() -> Result<Self> {
        let data_dir = Self::get_data_dir()?;
        let registry_path = data_dir.join("registry.json");

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        Ok(Self {
            registry_path,
            data_dir,
        })
    }

    /// Get the data directory for podman-bootc VMs
    pub fn get_data_dir() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Cannot determine cache directory"))?;
        Ok(cache_dir.join("bootc-kit").join("podman-bootc"))
    }

    /// Get the disk images directory
    pub fn get_disks_dir(&self) -> PathBuf {
        self.data_dir.join("disks")
    }

    /// Load registry from disk
    pub fn load_registry(&self) -> Result<VmRegistry> {
        if !self.registry_path.exists() {
            return Ok(VmRegistry::new());
        }

        let contents = std::fs::read_to_string(&self.registry_path).with_context(|| {
            format!(
                "Failed to read registry file: {}",
                self.registry_path.display()
            )
        })?;

        let registry: VmRegistry = serde_json::from_str(&contents).with_context(|| {
            format!(
                "Failed to parse registry file: {}",
                self.registry_path.display()
            )
        })?;

        Ok(registry)
    }

    /// Save registry to disk
    pub fn save_registry(&self, registry: &VmRegistry) -> Result<()> {
        let contents = serde_json::to_string_pretty(registry)
            .with_context(|| "Failed to serialize registry")?;

        std::fs::write(&self.registry_path, contents).with_context(|| {
            format!(
                "Failed to write registry file: {}",
                self.registry_path.display()
            )
        })?;

        Ok(())
    }

    /// Create disk path for a VM
    pub fn create_disk_path(&self, vm_name: &str) -> Result<PathBuf> {
        let disks_dir = self.get_disks_dir();
        std::fs::create_dir_all(&disks_dir).with_context(|| {
            format!("Failed to create disks directory: {}", disks_dir.display())
        })?;

        Ok(disks_dir.join(format!("{}.qcow2", vm_name)))
    }

    /// Remove VM disk and associated files
    pub fn cleanup_vm_files(&self, vm: &VmMetadata) -> Result<()> {
        // Remove disk image if it exists
        if vm.disk_path.exists() {
            std::fs::remove_file(&vm.disk_path).with_context(|| {
                format!("Failed to remove disk image: {}", vm.disk_path.display())
            })?;
        }

        Ok(())
    }

    /// Get registry path for debugging
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }

    /// Get data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_vm() -> VmMetadata {
        VmMetadata::new(
            "test-vm".to_string(),
            "quay.io/test/image:latest".to_string(),
            2048,
            2,
            20,
            "ext4".to_string(),
            "user".to_string(),
            vec!["8080:80".to_string()],
            vec!["/host:/guest".to_string()],
        )
    }

    #[test]
    fn test_vm_metadata_creation() {
        let vm = create_test_vm();
        assert_eq!(vm.name, "test-vm");
        assert_eq!(vm.image, "quay.io/test/image:latest");
        assert_eq!(vm.memory_mb, 2048);
        assert_eq!(vm.vcpus, 2);
        assert_eq!(vm.disk_size_gb, 20);
        assert_eq!(vm.filesystem, "ext4");
        assert_eq!(vm.network, "user");
        assert_eq!(vm.status, VmStatus::Created);
        assert!(vm.libvirt_domain.is_none());
        assert!(vm.ssh_port.is_none());
    }

    #[test]
    fn test_vm_status_updates() {
        let mut vm = create_test_vm();

        assert_eq!(vm.status, VmStatus::Created);
        assert!(!vm.is_running());
        assert!(!vm.is_stopped());
        assert!(!vm.has_error());

        vm.set_status(VmStatus::Running);
        assert!(vm.is_running());
        assert_eq!(vm.status_string(), "running");

        vm.set_status(VmStatus::Stopped);
        assert!(vm.is_stopped());
        assert_eq!(vm.status_string(), "stopped");

        vm.set_status(VmStatus::Error("Test error".to_string()));
        assert!(vm.has_error());
        assert_eq!(vm.status_string(), "error: Test error");
    }

    #[test]
    fn test_registry_operations() {
        let mut registry = VmRegistry::new();
        let vm = create_test_vm();

        // Test adding VM
        assert!(registry.add_vm(vm.clone()).is_ok());
        assert_eq!(registry.count(), 1);
        assert!(!registry.is_empty());

        // Test duplicate name
        assert!(registry.add_vm(vm.clone()).is_err());

        // Test getting VM
        assert!(registry.get_vm("test-vm").is_some());
        assert!(registry.get_vm("nonexistent").is_none());

        // Test listing
        let vms = registry.list_vms();
        assert_eq!(vms.len(), 1);
        assert_eq!(vms[0].name, "test-vm");

        // Test removal
        let removed = registry.remove_vm("test-vm");
        assert!(removed.is_some());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_name_generation() {
        let mut registry = VmRegistry::new();

        // Test simple name
        let name1 = registry.generate_vm_name("quay.io/test/image:latest");
        assert_eq!(name1, "image");

        // Add VM and test collision handling
        let vm = VmMetadata::new(
            name1.clone(),
            "quay.io/test/image:latest".to_string(),
            2048,
            2,
            20,
            "ext4".to_string(),
            "user".to_string(),
            vec![],
            vec![],
        );
        registry.add_vm(vm).unwrap();

        let name2 = registry.generate_vm_name("quay.io/test/image:latest");
        assert_eq!(name2, "image-2");

        // Test image without tag
        let name3 = registry.generate_vm_name("test-image");
        assert_eq!(name3, "test-image");

        // Test name with invalid characters
        let name4 = registry.generate_vm_name("test/image@sha256:abc123");
        assert_eq!(name4, "image-sha256");
    }

    #[test]
    fn test_latest_vm() {
        let mut registry = VmRegistry::new();

        // Empty registry
        assert!(registry.get_latest_vm().is_none());

        // Add first VM
        let vm1 = create_test_vm();
        registry.add_vm(vm1).unwrap();

        // Add second VM after a short delay
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mut vm2 = create_test_vm();
        vm2.name = "test-vm-2".to_string();
        registry.add_vm(vm2).unwrap();

        // Latest should be vm2
        let latest = registry.get_latest_vm().unwrap();
        assert_eq!(latest.name, "test-vm-2");
    }

    #[test]
    fn test_running_vms_filter() {
        let mut registry = VmRegistry::new();

        // Add VMs with different statuses
        let mut vm1 = create_test_vm();
        vm1.name = "vm1".to_string();
        vm1.set_status(VmStatus::Running);

        let mut vm2 = create_test_vm();
        vm2.name = "vm2".to_string();
        vm2.set_status(VmStatus::Stopped);

        let mut vm3 = create_test_vm();
        vm3.name = "vm3".to_string();
        vm3.set_status(VmStatus::Running);

        registry.add_vm(vm1).unwrap();
        registry.add_vm(vm2).unwrap();
        registry.add_vm(vm3).unwrap();

        // Test running VMs filter
        let running = registry.get_running_vms();
        assert_eq!(running.len(), 2);
        assert!(running.iter().any(|vm| vm.name == "vm1"));
        assert!(running.iter().any(|vm| vm.name == "vm3"));
        assert!(running.iter().all(|vm| vm.is_running()));
    }
}
