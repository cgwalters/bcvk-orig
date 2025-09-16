//! Common installation options shared across bcvk commands
//!
//! This module provides shared configuration structures for disk installation
//! operations, ensuring consistency across to-disk, libvirt-upload-disk,
//! and other installation-related commands.

use camino::Utf8PathBuf;
use clap::Parser;

/// Common installation options for bootc disk operations
///
/// These options control filesystem configuration and storage paths
/// for bootc installation commands. Use `#[clap(flatten)]` to include
/// these in command-specific option structures.
#[derive(Debug, Parser, Clone)]
pub struct InstallOptions {
    /// Root filesystem type (overrides bootc image default)
    #[clap(long, help = "Root filesystem type (e.g. ext4, xfs, btrfs)")]
    pub filesystem: Option<String>,

    /// Custom root filesystem size (e.g., '10G', '5120M')
    #[clap(long, help = "Root filesystem size (e.g., '10G', '5120M')")]
    pub root_size: Option<String>,

    /// Path to host container storage (auto-detected if not specified)
    #[clap(
        long,
        help = "Path to host container storage (auto-detected if not specified)"
    )]
    pub storage_path: Option<Utf8PathBuf>,
}

impl InstallOptions {
    /// Get the bootc install command arguments for these options
    pub fn to_bootc_args(&self) -> Vec<String> {
        let mut args = vec![];

        if let Some(ref filesystem) = self.filesystem {
            args.push("--filesystem".to_string());
            args.push(filesystem.clone());
        }

        if let Some(ref root_size) = self.root_size {
            args.push("--root-size".to_string());
            args.push(root_size.clone());
        }

        args
    }
}
