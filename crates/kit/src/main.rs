use std::ffi::OsString;

use cap_std_ext::cap_std::fs::Dir;
use clap::{Parser, Subcommand};
use color_eyre::{Report, Result};
use tracing::instrument;

pub(crate) mod containerenv;
mod envdetect;
mod hostexec;
mod images;
mod podman;
mod qemu;
mod run_ephemeral;
mod sshcred;
mod utils;
mod virtiofsd;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
struct HostExecOpts {
    /// Binary to run
    bin: OsString,

    /// Arguments to pass to the binary
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

#[derive(Subcommand)]
enum Commands {
    /// Execute a command in the host context
    Hostexec(HostExecOpts),
    /// Commands for bootc container imges
    #[clap(subcommand)]
    Images(images::ImagesOpts),
    /// Run a container image as an ephemeral VM with direct kernel boot
    RunEphemeral(run_ephemeral::RunEphemeralOpts),
    /// Code executed inside the target image
    #[clap(hide = true)]
    RunEphemeralImpl(run_ephemeral::RunEphemeralImplOpts),
    #[clap(hide = true)]
    DebugInternals(DebugInternalsOpts),
}

fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[instrument]
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
        Commands::RunEphemeralImpl(opts) => run_ephemeral::run_impl(opts)?,
        Commands::DebugInternals(opts) => match opts.command {
            DebugInternalsCmds::OpenTree { path } => {
                let fd = rustix::mount::open_tree(
                    rustix::fs::CWD,
                    path,
                    rustix::mount::OpenTreeFlags::OPEN_TREE_CLOEXEC
                        | rustix::mount::OpenTreeFlags::OPEN_TREE_CLONE,
                )?;
                let fd = Dir::reopen_dir(&fd)?;
                eprintln!("{:?}", fd.entries()?.into_iter().collect::<Vec<_>>());
            }
        },
    }
    Ok(())
}
