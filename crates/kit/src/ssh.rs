//! SSH integration for bcvk VMs

use camino::Utf8Path;
use color_eyre::{eyre::eyre, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::debug;

use crate::CONTAINER_STATEDIR;

/// Represents an SSH keypair with file paths and public key content
#[derive(Debug, Clone)]
pub struct SshKeyPair {
    /// Path to the private key file
    #[allow(dead_code)]
    pub private_key_path: PathBuf,
    /// Path to the public key file (typically private_key_path + ".pub")
    pub public_key_path: PathBuf,
}

/// Generate a new RSA SSH keypair in the specified directory
///
/// Creates a new 4096-bit RSA SSH keypair using the system's `ssh-keygen` command.
/// The private key is created with secure permissions (0600) and no passphrase to
/// enable automated use cases.
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
            &format!("bcvk-{}", key_name), // Comment
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

    debug!("Generated SSH keypair successfully");

    Ok(SshKeyPair {
        private_key_path,
        public_key_path,
    })
}

pub fn generate_default_keypair() -> Result<SshKeyPair> {
    generate_ssh_keypair(Path::new(CONTAINER_STATEDIR), "ssh")
}

/// Connect to VM via container-based SSH access
///
/// Establishes an SSH connection to a VM by executing SSH commands inside the
/// container that hosts the VM. This is the primary connection method for bcvk
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
pub fn connect_via_container(container_name: &str, args: Vec<String>) -> Result<()> {
    let status = connect_via_container_with_status(container_name, args)?;
    if !status.success() {
        return Err(eyre!(
            "SSH connection failed with exit code: {:?}",
            status.code()
        ));
    }
    Ok(())
}

/// SSH connection configuration options
#[derive(Debug, Clone)]
pub struct SshConnectionOptions {
    /// Connection timeout in seconds (default: 30)
    pub connect_timeout: u32,
    /// Enable/disable TTY allocation (default: true)
    pub allocate_tty: bool,
    /// SSH log level (default: ERROR)
    pub log_level: String,
    /// Additional SSH options as key-value pairs
    pub extra_options: Vec<(String, String)>,
    /// Suppress output to stdout/stderr (default: false)
    pub suppress_output: bool,
}

impl Default for SshConnectionOptions {
    fn default() -> Self {
        Self {
            connect_timeout: 30,
            allocate_tty: true,
            log_level: "ERROR".to_string(),
            extra_options: vec![],
            suppress_output: false,
        }
    }
}

impl SshConnectionOptions {
    /// Create options suitable for quick connectivity tests (short timeout, no TTY)
    pub fn for_connectivity_test() -> Self {
        Self {
            connect_timeout: 2,
            allocate_tty: false,
            log_level: "ERROR".to_string(),
            extra_options: vec![],
            suppress_output: true,
        }
    }
}

/// Connect to VM via container-based SSH access with configurable options
///
/// This is the most flexible SSH connection function that allows full control
/// over SSH options and connection parameters.
pub fn connect_via_container_with_options(
    container_name: &str,
    args: Vec<String>,
    options: &SshConnectionOptions,
) -> Result<std::process::ExitStatus> {
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
    if options.allocate_tty {
        cmd.args(["exec", "-it", container_name, "ssh"]);
    } else {
        cmd.args(["exec", container_name, "ssh"]);
    }

    // SSH key and security options
    let keypath = Utf8Path::new("/run/tmproot")
        .join(CONTAINER_STATEDIR.trim_start_matches('/'))
        .join("ssh");
    cmd.args(["-i", keypath.as_str()]);
    cmd.args(["-o", "IdentitiesOnly=yes"]);
    cmd.args(["-o", "PasswordAuthentication=no"]);
    cmd.args(["-o", "KbdInteractiveAuthentication=no"]);
    cmd.args(["-o", "GSSAPIAuthentication=no"]);
    cmd.args(["-o", "StrictHostKeyChecking=no"]);
    cmd.args(["-o", "UserKnownHostsFile=/dev/null"]);

    // Configurable options
    cmd.args(["-o", &format!("ConnectTimeout={}", options.connect_timeout)]);
    cmd.args(["-o", &format!("LogLevel={}", options.log_level)]);

    // Add extra SSH options
    for (key, value) in &options.extra_options {
        cmd.args(["-o", &format!("{}={}", key, value)]);
    }

    // Connect to VM via QEMU port forwarding on localhost
    cmd.arg(&format!("root@127.0.0.1"));
    cmd.args(["-p", "2222"]); // Use the forwarded port

    // Add any additional arguments
    if !args.is_empty() {
        cmd.arg("--");
        cmd.args(&args);
    }

    debug!("Executing: podman {:?}", cmd.get_args().collect::<Vec<_>>());

    // Suppress output if requested (useful for connectivity testing)
    if options.suppress_output {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    // Execute the command and return status
    cmd.status()
        .map_err(|e| eyre!("Failed to execute SSH command: {}", e))
}

/// Connect to VM via container-based SSH access, returning the exit status
///
/// Similar to `connect_via_container` but returns the process exit status
/// instead of an error when SSH exits with non-zero code. This is useful
/// for capturing the exit code of remote commands.
pub fn connect_via_container_with_status(
    container_name: &str,
    args: Vec<String>,
) -> Result<std::process::ExitStatus> {
    connect_via_container_with_options(container_name, args, &SshConnectionOptions::default())
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

        let content = std::fs::read_to_string(&key_pair.public_key_path).unwrap();
        // Check that public key starts with expected format
        assert!(content.starts_with("ssh-rsa"));

        // Check private key permissions
        let metadata = std::fs::metadata(&key_pair.private_key_path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_ssh_connection_options() {
        // Test default options
        let default_opts = SshConnectionOptions::default();
        assert_eq!(default_opts.connect_timeout, 30);
        assert!(default_opts.allocate_tty);
        assert_eq!(default_opts.log_level, "ERROR");
        assert!(default_opts.extra_options.is_empty());
        assert!(!default_opts.suppress_output);

        // Test connectivity test options
        let test_opts = SshConnectionOptions::for_connectivity_test();
        assert_eq!(test_opts.connect_timeout, 2);
        assert!(!test_opts.allocate_tty);
        assert_eq!(test_opts.log_level, "ERROR");
        assert!(test_opts.extra_options.is_empty());
        assert!(test_opts.suppress_output);

        // Test custom options
        let mut custom_opts = SshConnectionOptions::default();
        custom_opts.connect_timeout = 10;
        custom_opts.allocate_tty = false;
        custom_opts.log_level = "DEBUG".to_string();
        custom_opts
            .extra_options
            .push(("ServerAliveInterval".to_string(), "30".to_string()));

        assert_eq!(custom_opts.connect_timeout, 10);
        assert!(!custom_opts.allocate_tty);
        assert_eq!(custom_opts.log_level, "DEBUG");
        assert_eq!(custom_opts.extra_options.len(), 1);
        assert_eq!(
            custom_opts.extra_options[0],
            ("ServerAliveInterval".to_string(), "30".to_string())
        );
    }
}
