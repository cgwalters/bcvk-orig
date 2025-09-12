use std::fmt::Display;
use std::process::Command;

use color_eyre::eyre::{eyre, Context as _};
use color_eyre::Result;

/// A systemd version
#[derive(Debug, Clone)]
pub struct SystemdVersion(pub u32);

impl SystemdVersion {
    pub fn from_version_output(o: &str) -> Result<Self> {
        let Some(num) = o
            .lines()
            .next()
            .and_then(|v| v.split_ascii_whitespace().nth(1))
        else {
            return Err(eyre!("Failed to find systemd version"));
        };
        let num: u32 = num
            .parse()
            .with_context(|| format!("Parsing systemd version: {num}"))?;
        Ok(Self(num))
    }

    pub fn new_current() -> Result<Self> {
        let o = Command::new("systemctl").arg("--version").output()?;
        let o = String::from_utf8_lossy(&o.stdout);
        Self::from_version_output(&o)
    }

    /// https://www.freedesktop.org/software/systemd/man/latest/systemd.html#vmm.notify_socket
    pub fn has_vmm_notify(&self) -> bool {
        self.0 >= 254
    }
}

impl Display for SystemdVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "systemd {}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_systemd_version() {
        // Test typical systemd version output
        let output = "systemd 254 (254.5-1.fc39)\n+PAM +AUDIT +SELINUX";
        let version = SystemdVersion::from_version_output(output).unwrap();
        assert_eq!(version.0, 254);
        assert!(version.has_vmm_notify());

        // Test older version
        let output = "systemd 253 (253.1-1.fc38)\n+PAM +AUDIT +SELINUX";
        let version = SystemdVersion::from_version_output(output).unwrap();
        assert_eq!(version.0, 253);
        assert!(!version.has_vmm_notify());

        // Test different format (minimal)
        let output = "systemd 249";
        let version = SystemdVersion::from_version_output(output).unwrap();
        assert_eq!(version.0, 249);
        assert!(!version.has_vmm_notify());

        // Test newer version
        let output = "systemd 257 (257.7-1.fc42)";
        let version = SystemdVersion::from_version_output(output).unwrap();
        assert_eq!(version.0, 257);
        assert!(version.has_vmm_notify());
    }

    #[test]
    fn test_invalid_version_output() {
        let output = "";
        assert!(SystemdVersion::from_version_output(output).is_err());

        let output = "invalid output";
        assert!(SystemdVersion::from_version_output(output).is_err());

        let output = "systemd";
        assert!(SystemdVersion::from_version_output(output).is_err());
    }
}
