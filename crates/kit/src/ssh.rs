//! SSH integration for bootc-kit VMs
//!
//! This module provides comprehensive SSH functionality for connecting to and managing
//! bootc VMs. It handles SSH key generation, VM configuration management, and provides
//! multiple connection methods (container-based and host-based).
//!
//! # Architecture Overview
//!
//! The SSH integration works through several coordinated components:
//!
//! 1. **Key Generation**: RSA SSH keypairs are generated on-demand with secure permissions
//! 2. **Credential Injection**: Public keys are injected into VMs via SMBIOS credentials
//!    using systemd tmpfiles.d configuration
//! 3. **Connection Methods**: Two primary connection approaches:
//!    - Container-based: SSH via podman exec with keys mounted at `/tmp/ssh`
//!    - Host-based: Direct SSH to forwarded ports on localhost
//! 4. **VM Management**: Persistent configuration storage in user cache directories
//!
//! # Security Model
//!
//! - SSH private keys are generated with 0600 permissions (owner read/write only)
//! - No passphrases are used on generated keys for automation compatibility
//! - Keys are stored in isolated per-VM cache directories
//! - Host key checking is disabled for ephemeral VM connections
//! - Authentication methods are restricted to public key only
//!
//! # Connection Flow
//!
//! 1. SSH keypair is generated or loaded from existing identity
//! 2. Public key is encoded and injected into VM via systemd credentials
//! 3. VM boots with SSH access enabled on port 22
//! 4. QEMU forwards guest port 22 to host port 2222 (default)
//! 5. Connection is established either:
//!    - Via container: `podman exec -it <container> ssh -i /tmp/ssh root@127.0.0.1 -p 2222`
//!    - Via host: `ssh -i <key> root@localhost -p 2222`

use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

/// Represents an SSH keypair with file paths and public key content
///
/// This struct contains all the necessary information for an SSH keypair used
/// to access bootc VMs. The keypair is generated using RSA 4096-bit encryption
/// for enhanced security.
///
/// # Security Considerations
///
/// - Private keys are created with 0600 permissions (owner read/write only)
/// - No passphrase is set to enable automation use cases
/// - Public key content is cached to avoid repeated file I/O
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use bootc_kit::ssh::generate_ssh_keypair;
///
/// let keypair = generate_ssh_keypair(Path::new("/tmp"), "test_key")?;
/// println!("Private key: {:?}", keypair.private_key_path);
/// println!("Public key content: {}", keypair.public_key_content);
/// ```
#[derive(Debug, Clone)]
pub struct SshKeyPair {
    /// Path to the private key file
    pub private_key_path: PathBuf,
    /// Path to the public key file (typically private_key_path + ".pub")
    #[allow(dead_code)]
    pub public_key_path: PathBuf,
    /// Content of the public key as a string (cached for efficiency)
    pub public_key_content: String,
}

/// Configuration for SSH access to a specific VM
///
/// This struct stores the persistent configuration needed to connect to a bootc VM
/// via SSH. It supports both container-based and host-based connection methods.
/// The configuration is serialized to JSON and stored in the user's cache directory.
///
/// # Storage Location
///
/// Configurations are stored at: `~/.cache/bootc-kit/vm-{vm_id}/config.json`
///
/// # Connection Methods
///
/// - **Container-based**: Uses `container_name` to connect via `podman exec`
/// - **Host-based**: Connects directly to forwarded ports on localhost
///
/// # Example
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use bootc_kit::ssh::{VmSshConfig, save_vm_config};
///
/// let config = VmSshConfig {
///     vm_id: "abc123".to_string(),
///     ssh_key_path: PathBuf::from("/home/user/.cache/bootc-kit/vm-abc123/ssh_key"),
///     ssh_user: "root".to_string(),
///     container_name: Some("bootc-vm-abc123".to_string()),
/// };
///
/// save_vm_config(&config)?;
/// ```
#[derive(Serialize, Deserialize, Debug)]
pub struct VmSshConfig {
    /// Unique identifier for the VM
    pub vm_id: String,
    /// Path to the SSH private key used for authentication
    pub ssh_key_path: PathBuf,
    /// Username to use for SSH connections (typically "root")
    pub ssh_user: String,
    /// Optional container name for container-based connections
    pub container_name: Option<String>,
}

/// Generate a new RSA SSH keypair in the specified directory
///
/// Creates a new 4096-bit RSA SSH keypair using the system's `ssh-keygen` command.
/// The private key is created with secure permissions (0600) and no passphrase to
/// enable automated use cases.
///
/// # Arguments
///
/// * `output_dir` - Directory where the keypair will be created (created if it doesn't exist)
/// * `key_name` - Name for the key files (private key gets this name, public key gets ".pub" suffix)
///
/// # Returns
///
/// Returns an `SshKeyPair` struct containing paths to both keys and the public key content.
///
/// # Security Features
///
/// - Uses RSA 4096-bit keys for enhanced security
/// - Sets private key permissions to 0600 (owner read/write only)
/// - Adds a descriptive comment "bootc-kit-{key_name}" to the key
/// - No passphrase for automation compatibility
///
/// # Errors
///
/// Returns an error if:
/// - The output directory cannot be created
/// - `ssh-keygen` command fails
/// - File permissions cannot be set
/// - Public key content cannot be read
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use bootc_kit::ssh::generate_ssh_keypair;
///
/// let keypair = generate_ssh_keypair(Path::new("/tmp/ssh"), "vm_key")?;
/// println!("Generated keypair:");
/// println!("  Private: {:?}", keypair.private_key_path);
/// println!("  Public: {:?}", keypair.public_key_path);
/// println!("  Content: {}", keypair.public_key_content);
/// ```
pub fn generate_ssh_keypair(output_dir: &Path, key_name: &str) -> Result<SshKeyPair> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    let private_key_path = output_dir.join(key_name);
    let public_key_path = output_dir.join(format!("{}.pub", key_name));

    debug!("Generating SSH keypair at {:?}", private_key_path);

    // Generate RSA key with ssh-keygen
    let output = Command::new("ssh-keygen")
        .args([
            "-t",
            "rsa",
            "-b",
            "4096", // Use 4096-bit RSA for security
            "-f",
            private_key_path
                .to_str()
                .ok_or_else(|| eyre!("Invalid key path"))?,
            "-N",
            "", // No passphrase
            "-C",
            &format!("bootc-kit-{}", key_name), // Comment
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("ssh-keygen failed: {}", stderr));
    }

    // Set secure permissions on private key
    let metadata = fs::metadata(&private_key_path)?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o600); // Read/write for owner only
    fs::set_permissions(&private_key_path, permissions)?;

    // Read public key content
    let public_key_content = fs::read_to_string(&public_key_path)?;

    debug!("Generated SSH keypair successfully");

    Ok(SshKeyPair {
        private_key_path,
        public_key_path,
        public_key_content: public_key_content.trim().to_string(),
    })
}

/// Read SSH public key content from a private key file path
///
/// Given a path to an SSH private key, this function reads the corresponding
/// public key file. It handles the common case where public keys have a ".pub"
/// extension added to the private key filename.
///
/// This function is used when working with existing SSH identities provided via
/// the `--ssh-identity` option, allowing users to reuse their existing SSH keys
/// instead of generating new ones.
///
/// # Arguments
///
/// * `private_key_path` - Path to the SSH private key file
///
/// # Returns
///
/// Returns the public key content as a trimmed string.
///
/// # Key Resolution Logic
///
/// 1. If the private key path already ends with ".pub", use it directly
/// 2. Otherwise, append ".pub" to the private key path
/// 3. Read and return the trimmed content
///
/// # Errors
///
/// Returns an error if:
/// - The public key file doesn't exist
/// - The public key file cannot be read
/// - The file contains invalid UTF-8 content
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use bootc_kit::ssh::read_public_key;
///
/// // Read public key for existing private key
/// let pubkey_content = read_public_key(Path::new("/home/user/.ssh/id_rsa"))?;
/// println!("Public key: {}", pubkey_content);
///
/// // Also works if you accidentally pass the .pub path
/// let same_content = read_public_key(Path::new("/home/user/.ssh/id_rsa.pub"))?;
/// assert_eq!(pubkey_content, same_content);
/// ```
pub fn read_public_key(private_key_path: &Path) -> Result<String> {
    let public_key_path = private_key_path.with_extension("pub");
    if public_key_path.extension().is_none() {
        let mut path = private_key_path.to_path_buf();
        path.set_extension("pub");
        return read_public_key(&path);
    }

    let content = fs::read_to_string(&public_key_path)?;
    Ok(content.trim().to_string())
}

/// Get the cache directory for storing VM SSH configuration and keys
///
/// Returns the path to a VM-specific cache directory where SSH keys, configuration,
/// and other persistent data are stored. The directory follows the XDG Base Directory
/// specification and is created if it doesn't exist.
///
/// # Directory Structure
///
/// ```text
/// ~/.cache/bootc-kit/
/// └── vm-{vm_id}/
///     ├── config.json      # VM SSH configuration
///     ├── ssh_key          # Private SSH key
///     └── ssh_key.pub      # Public SSH key
/// ```
///
/// # Arguments
///
/// * `vm_id` - Unique identifier for the VM
///
/// # Returns
///
/// Returns the path to the VM's cache directory.
///
/// # Errors
///
/// Returns an error if:
/// - The user's cache directory cannot be determined
/// - The directory cannot be created due to permissions
///
/// # Example
///
/// ```rust,no_run
/// use bootc_kit::ssh::get_vm_cache_dir;
///
/// let vm_id = "abc123def456";
/// let cache_dir = get_vm_cache_dir(vm_id)?;
/// println!("VM cache directory: {:?}", cache_dir);
/// // Output: "/home/user/.cache/bootc-kit/vm-abc123def456"
/// ```
pub fn get_vm_cache_dir(vm_id: &str) -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| eyre!("Could not determine cache directory"))?
        .join("bootc-kit")
        .join(format!("vm-{}", vm_id));

    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

/// Generate a unique VM identifier
///
/// Creates a unique identifier for a VM based on the current timestamp.
/// The ID is used for isolating VM configurations, SSH keys, and other
/// resources in separate cache directories.
///
/// # ID Format
///
/// The generated ID is a hexadecimal string combining:
/// - Current Unix timestamp in seconds (hex encoded)
/// - Nanosecond component of the timestamp (hex encoded)
///
/// This ensures uniqueness even for VMs created in rapid succession.
///
/// # Returns
///
/// Returns a unique hexadecimal string identifier.
///
/// # Example
///
/// ```rust,no_run
/// use bootc_kit::ssh::generate_vm_id;
///
/// let vm_id1 = generate_vm_id();
/// let vm_id2 = generate_vm_id();
///
/// println!("VM ID 1: {}", vm_id1); // e.g., "66f1a3b2a1b2c3d4"
/// println!("VM ID 2: {}", vm_id2); // e.g., "66f1a3b2a1b2c3d5"
/// assert_ne!(vm_id1, vm_id2); // IDs are always unique
/// ```
///
/// # Usage
///
/// The generated ID is used to:
/// - Create isolated cache directories: `~/.cache/bootc-kit/vm-{vm_id}/`
/// - Label container resources: `bootc.kit.vm-id={vm_id}`
/// - Reference VMs in configuration files and logs
pub fn generate_vm_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let nanos = timestamp.subsec_nanos();
    let secs = timestamp.as_secs();
    format!("{:x}{:x}", secs, nanos)
}

/// Save VM SSH configuration to persistent storage
///
/// Serializes and saves a VM's SSH configuration to a JSON file in the VM's
/// cache directory. This allows the configuration to persist across sessions
/// and enables reconnection to existing VMs.
///
/// The configuration is stored at: `~/.cache/bootc-kit/vm-{vm_id}/config.json`
///
/// # Arguments
///
/// * `config` - The VM SSH configuration to save
///
/// # File Format
///
/// The configuration is saved as pretty-printed JSON:
/// ```json
/// {
///   "vm_id": "abc123def456",
///   "ssh_key_path": "/home/user/.cache/bootc-kit/vm-abc123def456/ssh_key",
///   "ssh_user": "root",
///   "container_name": "bootc-vm-abc123def456"
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The cache directory cannot be created
/// - The configuration cannot be serialized to JSON
/// - The file cannot be written due to permissions
///
/// # Example
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use bootc_kit::ssh::{VmSshConfig, save_vm_config};
///
/// let config = VmSshConfig {
///     vm_id: "test123".to_string(),
///     ssh_key_path: PathBuf::from("/tmp/test_key"),
///     ssh_user: "root".to_string(),
///     container_name: Some("test-vm".to_string()),
/// };
///
/// save_vm_config(&config)?;
/// println!("Configuration saved successfully");
/// ```
pub fn save_vm_config(config: &VmSshConfig) -> Result<()> {
    let cache_dir = get_vm_cache_dir(&config.vm_id)?;
    let config_path = cache_dir.join("config.json");

    let json = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, json)?;

    debug!("Saved VM SSH config to {:?}", config_path);
    Ok(())
}

/// Load VM SSH configuration from persistent storage
///
/// Reads and deserializes a VM's SSH configuration from the JSON file in the
/// VM's cache directory. This enables reconnection to existing VMs that were
/// previously configured.
///
/// # Arguments
///
/// * `vm_id` - Unique identifier for the VM whose configuration to load
///
/// # Returns
///
/// Returns the deserialized `VmSshConfig` struct.
///
/// # Errors
///
/// Returns an error if:
/// - The VM's cache directory doesn't exist
/// - The configuration file doesn't exist (VM not found)
/// - The file cannot be read due to permissions
/// - The JSON content is malformed or incompatible
///
/// # Example
///
/// ```rust,no_run
/// use bootc_kit::ssh::{load_vm_config, save_vm_config, VmSshConfig};
/// use std::path::PathBuf;
///
/// // Save a configuration
/// let config = VmSshConfig {
///     vm_id: "test123".to_string(),
///     ssh_key_path: PathBuf::from("/tmp/test_key"),
///     ssh_user: "root".to_string(),
///     container_name: Some("test-vm".to_string()),
/// };
/// save_vm_config(&config)?;
///
/// // Load it back
/// let loaded_config = load_vm_config("test123")?;
/// assert_eq!(config.vm_id, loaded_config.vm_id);
/// assert_eq!(config.ssh_user, loaded_config.ssh_user);
/// ```
///
/// # Usage Pattern
///
/// This function is typically used in workflows like:
/// 1. User runs `bootc-kit ssh <vm_id>`
/// 2. System loads the configuration to find SSH key path and connection method
/// 3. Connection is established using the stored configuration
#[allow(dead_code)]
pub fn load_vm_config(vm_id: &str) -> Result<VmSshConfig> {
    let cache_dir = get_vm_cache_dir(vm_id)?;
    let config_path = cache_dir.join("config.json");

    if !config_path.exists() {
        return Err(eyre!("VM config not found for ID: {}", vm_id));
    }

    let json = fs::read_to_string(&config_path)?;
    let config: VmSshConfig = serde_json::from_str(&json)?;

    debug!("Loaded VM SSH config from {:?}", config_path);
    Ok(config)
}

/// Connect to VM via container-based SSH access
///
/// Establishes an SSH connection to a VM by executing SSH commands inside the
/// container that hosts the VM. This is the primary connection method for bootc-kit
/// VMs and provides isolated, secure access without requiring direct host network
/// configuration.
///
/// # Connection Architecture
///
/// ```text
/// Host → Podman Container → SSH → VM (localhost:2222)
///        │                │
///        └─ SSH Key at    └─ QEMU port forwarding
///           /tmp/ssh         (guest:22 → host:2222)
/// ```
///
/// # Arguments
///
/// * `container_name` - Name of the podman container hosting the VM
/// * `_ssh_key` - Path to SSH private key (unused - key is mounted at /tmp/ssh)
/// * `ssh_user` - Username for SSH connection (typically "root")
/// * `args` - Additional arguments to pass to the SSH command
///
/// # Container Requirements
///
/// This function requires:
/// - Container exists and is in "running" state  
/// - SSH private key is mounted at `/tmp/ssh` inside the container
/// - QEMU is configured with port forwarding (guest:22 → host:2222)
/// - SSH client is available inside the container
///
/// # Connection Process
///
/// 1. **Container Verification**: Checks if container exists and is running
/// 2. **SSH Execution**: Runs `podman exec -it <container> ssh ...`
/// 3. **Key Authentication**: Uses the key mounted at `/tmp/ssh`
/// 4. **Port Forwarding**: Connects to 127.0.0.1:2222 (QEMU forwarding)
///
/// # SSH Configuration
///
/// The function configures SSH with secure, VM-appropriate settings:
/// - Uses only the mounted key (`-i /tmp/ssh`)
/// - Disables all other authentication methods
/// - Skips host key checking (ephemeral VMs)
/// - Reduces log verbosity to ERROR level
///
/// # Errors
///
/// Returns an error if:
/// - Container doesn't exist or isn't running
/// - Podman exec command fails
/// - SSH connection to VM fails
/// - VM's SSH service isn't accessible
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use bootc_kit::ssh::connect_via_container;
///
/// // Interactive SSH session
/// let key_path = Path::new("/tmp/unused"); // Key is mounted in container
/// connect_via_container("bootc-vm-abc123", key_path, "root", vec![])?;
///
/// // Run a specific command
/// let args = vec!["systemctl".to_string(), "status".to_string()];
/// connect_via_container("bootc-vm-abc123", key_path, "root", args)?;
/// ```
///
/// # Generated Command Structure
///
/// The function generates a command like:
/// ```bash
/// podman exec -it bootc-vm-abc123 ssh \
///     -i /tmp/ssh \
///     -o IdentitiesOnly=yes \
///     -o PasswordAuthentication=no \
///     -o KbdInteractiveAuthentication=no \
///     -o GSSAPIAuthentication=no \
///     -o StrictHostKeyChecking=no \
///     -o UserKnownHostsFile=/dev/null \
///     -o LogLevel=ERROR \
///     root@127.0.0.1 -p 2222 \
///     -- [additional args]
/// ```
///
/// # Security Notes
///
/// - SSH key is isolated within the container environment
/// - No host networking configuration required  
/// - Container provides additional isolation layer
/// - VM network access is controlled by QEMU configuration
pub fn connect_via_container(
    container_name: &str,
    _ssh_key: &Path,
    ssh_user: &str,
    args: Vec<String>,
) -> Result<()> {
    debug!("Connecting to VM via container: {}", container_name);

    // Verify container exists and is running
    let status = Command::new("podman")
        .args(["inspect", container_name, "--format", "{{.State.Status}}"])
        .output()
        .map_err(|e| eyre!("Failed to check container status: {}", e))?;

    if !status.status.success() {
        return Err(eyre!("Container '{}' not found", container_name));
    }

    let container_status = String::from_utf8_lossy(&status.stdout).trim().to_string();
    if container_status != "running" {
        return Err(eyre!(
            "Container '{}' is not running (status: {})",
            container_name,
            container_status
        ));
    }

    // Build podman exec command
    let mut cmd = Command::new("podman");
    cmd.args(["exec", "-it", container_name, "ssh"]);

    // Add SSH options
    cmd.args(["-i", "/tmp/ssh"]);
    cmd.args(["-o", "IdentitiesOnly=yes"]);
    cmd.args(["-o", "PasswordAuthentication=no"]);
    cmd.args(["-o", "KbdInteractiveAuthentication=no"]);
    cmd.args(["-o", "GSSAPIAuthentication=no"]);
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    cmd.args(["-o", "UserKnownHostsFile=/dev/null"]);
    cmd.args(["-o", "LogLevel=ERROR"]); // Reduce SSH verbosity

    // Connect to VM via QEMU port forwarding on localhost
    cmd.arg(&format!("{}@127.0.0.1", ssh_user));
    cmd.args(["-p", "2222"]); // Use the forwarded port

    // Add any additional arguments
    if !args.is_empty() {
        cmd.arg("--");
        cmd.args(&args);
    }

    debug!("Executing: podman {:?}", cmd.get_args().collect::<Vec<_>>());

    // Execute the command
    let status = cmd.status()?;

    if !status.success() {
        return Err(eyre!(
            "SSH connection failed with exit code: {:?}",
            status.code()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_ssh_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let key_pair = generate_ssh_keypair(temp_dir.path(), "test_key").unwrap();

        // Check that files exist
        assert!(key_pair.private_key_path.exists());
        assert!(key_pair.public_key_path.exists());

        // Check that public key content is not empty
        assert!(!key_pair.public_key_content.is_empty());

        // Check that public key starts with expected format
        assert!(key_pair.public_key_content.starts_with("ssh-rsa"));

        // Check private key permissions
        let metadata = std::fs::metadata(&key_pair.private_key_path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_read_public_key() {
        let temp_dir = TempDir::new().unwrap();
        let key_pair = generate_ssh_keypair(temp_dir.path(), "test_key").unwrap();

        let public_key_content = read_public_key(&key_pair.private_key_path).unwrap();
        assert_eq!(public_key_content, key_pair.public_key_content);
    }

    #[test]
    fn test_vm_id_generation() {
        let vm_id1 = generate_vm_id();

        // Sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        let vm_id2 = generate_vm_id();

        // IDs should be different and non-empty
        assert_ne!(vm_id1, vm_id2);
        assert!(!vm_id1.is_empty());
        assert!(!vm_id2.is_empty());
    }

    #[test]
    fn test_vm_config_save_load() {
        let vm_id = generate_vm_id();
        let config = VmSshConfig {
            vm_id: vm_id.clone(),
            ssh_key_path: PathBuf::from("/test/key"),
            ssh_user: "root".to_string(),
            container_name: Some("test-container".to_string()),
        };

        save_vm_config(&config).unwrap();
        let loaded_config = load_vm_config(&vm_id).unwrap();

        assert_eq!(config.vm_id, loaded_config.vm_id);
        assert_eq!(config.ssh_key_path, loaded_config.ssh_key_path);
        assert_eq!(config.ssh_user, loaded_config.ssh_user);
        assert_eq!(config.container_name, loaded_config.container_name);
    }
}
