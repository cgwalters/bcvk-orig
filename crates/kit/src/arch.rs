//! Architecture detection and configuration utilities
//!
//! This module provides cross-architecture support for libvirt domain creation
//! and QEMU emulator selection, avoiding hardcoded architecture assumptions.

use color_eyre::Result;

/// Architecture configuration for libvirt domains and QEMU
#[derive(Debug, Clone)]
pub struct ArchConfig {
    /// Architecture string for libvirt (e.g., "x86_64", "aarch64")
    pub arch: &'static str,
    /// Machine type for libvirt (e.g., "q35", "virt")
    pub machine: &'static str,
    /// QEMU emulator binary path
    pub emulator: &'static str,
    /// OS type for libvirt (usually "hvm")
    pub os_type: &'static str,
}

impl ArchConfig {
    /// Detect host architecture and return appropriate configuration
    pub fn detect() -> Result<Self> {
        let arch = std::env::consts::ARCH;
        match arch {
            "x86_64" => Ok(Self {
                arch: "x86_64",
                machine: "q35",
                emulator: "/usr/bin/qemu-system-x86_64",
                os_type: "hvm",
            }),
            "aarch64" => Ok(Self {
                arch: "aarch64",
                machine: "virt",
                emulator: "/usr/bin/qemu-system-aarch64",
                os_type: "hvm",
            }),
            // Add more architectures as needed
            // "riscv64" => Ok(Self {
            //     arch: "riscv64",
            //     machine: "virt",
            //     emulator: "/usr/bin/qemu-system-riscv64",
            //     os_type: "hvm",
            // }),
            unsupported => Err(color_eyre::eyre::eyre!(
                "Unsupported architecture: {}. Supported architectures: x86_64, aarch64",
                unsupported
            )),
        }
    }

    /// Check if the QEMU emulator exists on the system
    pub fn validate_emulator(&self) -> Result<()> {
        if !std::path::Path::new(self.emulator).exists() {
            return Err(color_eyre::eyre::eyre!(
                "QEMU emulator not found: {}. Please install the appropriate QEMU package for {} architecture.",
                self.emulator,
                self.arch
            ));
        }
        Ok(())
    }

    /// Get architecture-specific XML features for libvirt
    pub fn xml_features(&self) -> &'static str {
        match self.arch {
            "x86_64" => {
                r#"
  <features>
    <acpi/>
    <apic/>
    <vmport state='off'/>
  </features>"#
            }
            "aarch64" => {
                r#"
  <features>
    <acpi/>
    <apic/>
  </features>"#
            }
            _ => {
                r#"
  <features>
    <acpi/>
    <apic/>
  </features>"#
            }
        }
    }

    /// Get architecture-specific timer configuration
    pub fn xml_timers(&self) -> &'static str {
        match self.arch {
            "x86_64" => {
                r#"
    <timer name='rtc' tickpolicy='catchup'/>
    <timer name='pit' tickpolicy='delay'/>
    <timer name='hpet' present='no'/>"#
            }
            "aarch64" => {
                r#"
    <timer name='rtc' tickpolicy='catchup'/>"#
            }
            _ => {
                r#"
    <timer name='rtc' tickpolicy='catchup'/>"#
            }
        }
    }

    /// Check if this architecture supports VMport (x86_64 specific feature)
    #[allow(dead_code)]
    pub fn supports_vmport(&self) -> bool {
        self.arch == "x86_64"
    }

    /// Get recommended CPU mode for this architecture
    pub fn cpu_mode(&self) -> &'static str {
        match self.arch {
            "x86_64" => "host-passthrough",
            "aarch64" => "host-passthrough",
            _ => "host-model",
        }
    }
}

/// Detect host architecture string (shorthand for ArchConfig::detect().arch)
#[allow(dead_code)]
pub fn host_arch() -> Result<&'static str> {
    Ok(ArchConfig::detect()?.arch)
}

/// Check if running on x86_64 architecture
#[allow(dead_code)]
pub fn is_x86_64() -> bool {
    std::env::consts::ARCH == "x86_64"
}

/// Check if running on ARM64/AArch64 architecture  
#[allow(dead_code)]
pub fn is_aarch64() -> bool {
    std::env::consts::ARCH == "aarch64"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_detection() {
        let arch_config = ArchConfig::detect().unwrap();

        // Should detect the current architecture
        assert_eq!(arch_config.arch, std::env::consts::ARCH);

        // Should have valid configuration
        assert!(!arch_config.machine.is_empty());
        assert!(!arch_config.emulator.is_empty());
        assert_eq!(arch_config.os_type, "hvm");
    }

    #[test]
    fn test_arch_specific_features() {
        let arch_config = ArchConfig::detect().unwrap();

        // All architectures should have some features
        assert!(!arch_config.xml_features().is_empty());
        assert!(!arch_config.xml_timers().is_empty());

        // CPU mode should be valid
        assert!(!arch_config.cpu_mode().is_empty());
    }

    #[test]
    fn test_vmport_support() {
        let arch_config = ArchConfig::detect().unwrap();

        // VMport support should match architecture
        if arch_config.arch == "x86_64" {
            assert!(arch_config.supports_vmport());
        } else {
            assert!(!arch_config.supports_vmport());
        }
    }

    #[test]
    fn test_helper_functions() {
        let detected_arch = host_arch().unwrap();
        assert_eq!(detected_arch, std::env::consts::ARCH);

        // At least one should be true
        assert!(is_x86_64() || is_aarch64());

        // Should be mutually exclusive
        assert!(!(is_x86_64() && is_aarch64()));
    }
}
