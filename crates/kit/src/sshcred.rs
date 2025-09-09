//! SSH credential injection for bootc VMs
//!
//! Injects SSH public keys into VMs via systemd credentials using either SMBIOS
//! firmware variables (preferred) or kernel command-line arguments. Creates systemd
//! tmpfiles.d configuration to set up SSH access during VM boot.
//!

use color_eyre::Result;

/// Generate SMBIOS credential string for root SSH access
///
/// Creates a systemd credential for QEMU's SMBIOS interface. Preferred method
/// as it keeps credentials out of kernel command line and boot logs.
///
/// Returns a string for use with `qemu -smbios type=11,value="..."`
pub fn smbios_cred_for_root_ssh(pubkey: &str) -> Result<String> {
    let k = key_to_root_tmpfiles_d(pubkey);
    let encoded = data_encoding::BASE64.encode(k.as_bytes());
    let r = format!("io.systemd.credential.binary:tmpfiles.extra={encoded}");
    Ok(r)
}

/// Generate kernel command-line argument for root SSH access
///
/// Creates a systemd credential for kernel command-line delivery. Less secure
/// than SMBIOS method as credentials are visible in /proc/cmdline and boot logs.
///
/// Returns a string for use in kernel boot parameters.
pub fn karg_for_root_ssh(pubkey: &str) -> Result<String> {
    let k = key_to_root_tmpfiles_d(pubkey);
    let encoded = data_encoding::BASE64.encode(k.as_bytes());
    let r = format!("systemd.set_credential_binary=tmpfiles.extra:{encoded}");
    Ok(r)
}

/// Convert SSH public key to systemd tmpfiles.d configuration
///
/// Generates configuration to create `/root/.ssh` directory (0750) and
/// `/root/.ssh/authorized_keys` file (700) with the Base64-encoded SSH key.
/// Uses `f+~` to append to existing authorized_keys files.
pub fn key_to_root_tmpfiles_d(pubkey: &str) -> String {
    let buf = data_encoding::BASE64.encode(pubkey.as_bytes());
    format!("d /root/.ssh 0750 - - -\nf+~ /root/.ssh/authorized_keys 700 - - - {buf}")
}

/// Generate SMBIOS credential string for AF_VSOCK systemd notification socket
///
/// Creates a systemd credential that configures systemd to send notifications
/// via AF_VSOCK instead of the default Unix socket. This enables host-guest
/// communication for debugging VM boot sequences.
///
/// Returns a string for use with `qemu -smbios type=11,value="..."`
pub fn smbios_cred_for_vsock_notify(host_cid: u32, port: u32) -> String {
    format!(
        "io.systemd.credential:vmm.notify_socket=vsock-stream:{}:{}",
        host_cid, port
    )
}

#[cfg(test)]
mod tests {
    use data_encoding::BASE64;
    use similar_asserts::assert_eq;

    use super::*;

    /// Test SSH public key for validation (truncated for brevity)
    const STUBKEY: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC...";

    /// Test tmpfiles.d configuration generation
    #[test]
    fn test_key_to_root_tmpfiles_d() {
        let expected = "d /root/.ssh 0750 - - -\nf+~ /root/.ssh/authorized_keys 700 - - - c3NoLXJzYSBBQUFBQjNOemFDMXljMkVBQUFBREFRQUJBQUFCQVFDLi4u";
        assert_eq!(key_to_root_tmpfiles_d(STUBKEY), expected);
    }

    /// Test SMBIOS credential generation and format validation
    #[test]
    fn test_credential_for_root_ssh() {
        let b64_tmpfiles = BASE64.encode(key_to_root_tmpfiles_d(STUBKEY).as_bytes());
        let expected = format!("io.systemd.credential.binary:tmpfiles.extra={b64_tmpfiles}");

        // Verify credential format by reverse parsing
        let v = expected
            .strip_prefix("io.systemd.credential.binary:")
            .unwrap();
        let v = v.strip_prefix("tmpfiles.extra=").unwrap();
        let v = String::from_utf8(BASE64.decode(v.as_bytes()).unwrap()).unwrap();
        assert_eq!(v, "d /root/.ssh 0750 - - -\nf+~ /root/.ssh/authorized_keys 700 - - - c3NoLXJzYSBBQUFBQjNOemFDMXljMkVBQUFBREFRQUJBQUFCQVFDLi4u");

        // Test the actual function output
        assert_eq!(smbios_cred_for_root_ssh(STUBKEY).unwrap(), expected);
    }
}
