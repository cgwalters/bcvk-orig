use std::ffi::OsString;

use cap_std_ext::cap_std::fs::Dir;
use clap::{Parser, Subcommand};
use color_eyre::{Report, Result};

mod arch;
mod cli_json;
mod container_entrypoint;
pub(crate) mod containerenv;
mod envdetect;
mod hostexec;
mod images;
mod install_options;
mod libvirt;
mod libvirt_upload_disk;
#[allow(dead_code)]
mod podman;
mod podman_bootc;
#[allow(dead_code)]
mod qemu;
mod run_ephemeral;
mod run_ephemeral_ssh;
mod ssh;
#[allow(dead_code)]
mod sshcred;
pub(crate) mod systemd;
mod to_disk;
mod utils;

pub const CONTAINER_STATEDIR: &str = "/var/lib/bcvk";

/// A comprehensive toolkit for bootc containers and local virtualization.
///
/// bcvk provides a complete workflow for building, testing, and managing
/// bootc containers using ephemeral VMs. Run bootc images as temporary VMs,
/// install them to disk, or manage existing installations - all without
/// requiring root privileges.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Execute a command in the host context from within a container.
///
/// This allows containers to run host commands with proper isolation
/// and resource management through the host execution system.
#[derive(Parser)]
struct HostExecOpts {
    /// Binary executable to run on the host system
    ///
    /// Can be a full path or a command name available in PATH.
    bin: OsString,

    /// Command-line arguments to pass to the binary
    ///
    /// All arguments after the binary name, including flags and options.
    /// Supports arguments starting with hyphens.
    #[clap(allow_hyphen_values = true)]
    args: Vec<OsString>,
}

#[derive(Parser)]
struct DebugInternalsOpts {
    #[command(subcommand)]
    command: DebugInternalsCmds,
}

#[derive(Subcommand)]
enum DebugInternalsCmds {
    OpenTree { path: std::path::PathBuf },
}

/// Internal diagnostic and tooling commands for development
#[derive(Parser)]
struct InternalsOpts {
    #[command(subcommand)]
    command: InternalsCmds,
}

#[derive(Subcommand)]
enum InternalsCmds {
    /// Dump CLI structure as JSON for man page generation
    #[cfg(feature = "docgen")]
    DumpCliJson,
}

/// SSH connection options for accessing running VMs.
///
/// Provides secure shell access to VMs running within containers,
/// with automatic key management and connection routing.
#[derive(Parser)]
struct SshOpts {
    /// Name or ID of the container running the target VM
    ///
    /// This should match the container name from podman or the VM ID
    /// used when starting the ephemeral VM.
    container_name: String,

    /// Additional SSH client arguments to pass through
    ///
    /// Standard ssh arguments like -v for verbose output, -L for
    /// port forwarding, or -o for SSH options.
    #[clap(allow_hyphen_values = true, help = "SSH arguments like -v, -L, -o")]
    args: Vec<String>,
}

/// Available bcvk commands for container and VM management.
#[derive(Subcommand)]
enum Commands {
    /// Execute commands on the host system from within containers
    #[clap(hide = true)]
    Hostexec(HostExecOpts),

    /// Manage and inspect bootc container images
    #[clap(subcommand)]
    Images(images::ImagesOpts),

    /// Run bootc containers as temporary VMs
    #[clap(name = "run-ephemeral")]
    RunEphemeral(run_ephemeral::RunEphemeralOpts),

    /// Run ephemeral VM and SSH into it
    #[clap(name = "run-ephemeral-ssh")]
    RunEphemeralSsh(run_ephemeral_ssh::RunEphemeralSshOpts),

    /// Install bootc images to persistent disk images
    #[clap(name = "to-disk")]
    ToDisk(to_disk::ToDiskOpts),

    /// Manage libvirt integration for bootc containers
    #[clap(subcommand)]
    Libvirt(libvirt::LibvirtCommands),

    /// Upload bootc disk images to libvirt (deprecated)
    #[clap(name = "libvirt-upload-disk", hide = true)]
    LibvirtUploadDisk(libvirt_upload_disk::LibvirtUploadDiskOpts),

    /// Connect to running VMs via SSH
    Ssh(SshOpts),

    /// Podman-bootc drop-in replacement commands
    #[clap(name = "pb")]
    PodmanBootc(podman_bootc::PodmanBootcOpts),

    /// Internal container entrypoint command (hidden from help)
    #[clap(hide = true)]
    ContainerEntrypoint(container_entrypoint::ContainerEntrypointOpts),

    /// Internal debugging and diagnostic tools (hidden from help)
    #[clap(hide = true)]
    DebugInternals(DebugInternalsOpts),

    /// Internal diagnostic and tooling commands for development
    #[clap(hide = true)]
    Internals(InternalsOpts),
}

/// Install and configure the tracing/logging system.
///
/// Sets up structured logging with environment-based filtering,
/// error layer integration, and console output formatting.
/// Logs are filtered by RUST_LOG environment variable, defaulting to 'info'.
fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

/// Main entry point for the bcvk CLI application.
///
/// Initializes logging, error handling, and command dispatch for all
/// bcvk operations including VM management, SSH access, and
/// container image handling.
fn main() -> Result<(), Report> {
    install_tracing();
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Hostexec(opts) => {
            hostexec::run(opts.bin, opts.args)?;
        }
        Commands::Images(opts) => opts.run()?,
        Commands::RunEphemeral(opts) => {
            run_ephemeral::run(opts)?;
        }
        Commands::RunEphemeralSsh(opts) => {
            run_ephemeral_ssh::run_ephemeral_ssh(opts)?;
        }
        Commands::ToDisk(opts) => {
            to_disk::run(opts)?;
        }
        Commands::Libvirt(cmd) => {
            cmd.run()?;
        }
        Commands::LibvirtUploadDisk(opts) => {
            eprintln!(
                "Warning: 'libvirt-upload-disk' is deprecated. Use 'libvirt upload' instead."
            );
            libvirt_upload_disk::run(opts)?;
        }
        Commands::Ssh(opts) => {
            // Wait for systemd to signal readiness or fall back to SSH polling
            let has_systemd_notify = run_ephemeral_ssh::wait_for_systemd_ready(
                &opts.container_name,
                std::time::Duration::from_secs(60),
            )?;

            if !has_systemd_notify {
                // Fall back to SSH polling for older systemd versions
                run_ephemeral_ssh::wait_for_ssh_ready(
                    &opts.container_name,
                    std::time::Duration::from_secs(60),
                )?;
            }

            ssh::connect_via_container(&opts.container_name, opts.args)?;
        }
        Commands::PodmanBootc(opts) => {
            opts.run()?;
        }
        Commands::ContainerEntrypoint(opts) => {
            // Create a tokio runtime for async container entrypoint operations
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(container_entrypoint::run(opts))?;
        }
        Commands::DebugInternals(opts) => match opts.command {
            DebugInternalsCmds::OpenTree { path } => {
                let fd = rustix::mount::open_tree(
                    rustix::fs::CWD,
                    path,
                    rustix::mount::OpenTreeFlags::OPEN_TREE_CLOEXEC
                        | rustix::mount::OpenTreeFlags::OPEN_TREE_CLONE,
                )?;
                let fd = Dir::reopen_dir(&fd)?;
                tracing::debug!("{:?}", fd.entries()?.into_iter().collect::<Vec<_>>());
            }
        },
        Commands::Internals(opts) => match opts.command {
            #[cfg(feature = "docgen")]
            InternalsCmds::DumpCliJson => {
                let json = cli_json::dump_cli_json()?;
                println!("{}", json);
            }
        },
    }
    Ok(())
}
