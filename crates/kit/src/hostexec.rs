//! Host command execution from containerized environments
//!
//! Executes commands on the host system from containers using systemd-run.
//! Commands are bound to container lifecycle for automatic cleanup.
//! Requires --privileged and --pid=host container flags.
//!
use std::ffi::OsStr;
use std::io::BufRead;
use std::os::unix::ffi::OsStrExt;
use std::process::Command;
use std::{collections::HashMap, ffi::OsString};

use bootc_utils::CommandRunExt;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use rand::distr::SampleString;

use crate::containerenv::ContainerExecutionInfo;

/// Configuration for systemd-run execution
#[derive(Debug, Default)]
pub struct SystemdConfig {
    /// Run detached without output capture (default: false uses --pipe)
    detached: bool,
}

/// Validate host execution environment
///
/// Returns None for host system (direct exec), Some(info) for valid container
/// with --privileged and --pid=host, or error for insufficient privileges.
fn ensure_hostexec_initialized() -> Result<Option<&'static ContainerExecutionInfo>> {
    // Check if we're in a toolbox environment - if so, we're on the host
    if std::env::var("TOOLBOX_PATH").is_ok() {
        return Ok(None);
    }

    let hostenv = crate::envdetect::Environment::get_cached()?;
    if !hostenv.container {
        return Ok(None);
    }
    let Some(info) = hostenv.containerenv.as_ref() else {
        return Err(eyre!("This command requires running with --privileged"));
    };
    if !hostenv.privileged {
        return Err(eyre!("This command requires running with --privileged"));
    }
    // This should be filled if run with --privileged and we're in a container
    if !hostenv.pidhost {
        return Err(eyre!("This command requires running with --pid=host"));
    }

    Ok(Some(info))
}

/// Create Command for host execution
///
/// Returns direct Command for host systems, or systemd-run wrapped Command
/// for containers. Generated service units are bound to container lifecycle.
pub fn command(exe: impl AsRef<OsStr>, config: Option<SystemdConfig>) -> Result<Command> {
    let exe = exe.as_ref();
    let config = config.unwrap_or_default();

    let Some(info) = ensure_hostexec_initialized()? else {
        return Ok(Command::new(exe));
    };

    let containerid = &info.id;
    // A random suffix, 8 alphanumeric chars gives 62 ** 8 possibilities, so low chance of collision
    // And we only care about such collissions for *concurrent* processes bound to *the same*
    // podman container ID; after a unit has exited it's fine if we reuse an ID.
    let runid = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 8);
    let unit = format!("hostcmd-{containerid}-{runid}.service");
    let scope = format!("libpod-{containerid}.scope");
    let properties = [format!("BindsTo={scope}"), format!("After={scope}")];

    let properties = properties.into_iter().flat_map(|p| ["-p".to_owned(), p]);
    let mut r = Command::new("systemd-run");
    // Note that we need to specify this ExecSearchPath property to suppress heuristics
    // systemd-run has to search for the binary, which in the general case won't exist
    // in the container.
    r.args([
        "--quiet",
        "--collect",
        "-u",
        unit.as_str(),
        "--property=ExecSearchPath=/usr/bin",
    ]);
    if !config.detached {
        r.arg("--pipe");
    }
    if info.rootless.is_some() {
        r.arg("--user");
    }
    r.args(properties);
    r.arg("--");
    r.arg(exe);
    Ok(r)
}

/// Execute command on host with error handling
///
/// Convenience function that creates, executes, and validates command exit status.
/// Inherits stdin/stdout/stderr from calling process.
pub fn run<I, T>(exe: impl AsRef<OsStr>, args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut c = command(exe, None)?;
    c.args(args.into_iter().map(|c| c.into()));
    c.run().map_err(|e| eyre!("{e:?}"))
}

/// Create podman command for host execution
///
/// Convenience wrapper around `command("podman", None)` for container
/// management operations that need host access.
#[allow(dead_code)]
pub fn podman() -> Result<Command> {
    command("podman", None)
}

/// Parse environment variables from KEY=VALUE format
///
/// Parses output from `env` command into HashMap. Handles binary data
/// and multiple `=` characters in values.
#[allow(dead_code)]
fn parse_env(e: impl BufRead) -> Result<HashMap<OsString, OsString>> {
    e.split(b'\n').try_fold(HashMap::new(), |mut r, line| {
        let line = line?;
        let mut split = line.split(|&c| c == b'=');
        let Some(k) = split.next() else {
            return Ok(r);
        };
        let Some(v) = split.next() else {
            return Ok(r);
        };
        r.insert(
            OsStr::from_bytes(k).to_owned(),
            OsStr::from_bytes(v).to_owned(),
        );
        Ok(r)
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// Test environment variable parsing
    #[test]
    fn test_parse_env() {
        let input = b"FOO=bar\nBAZ=quux\n";
        let expected: HashMap<OsString, OsString> = [
            (OsStr::new("FOO"), OsStr::new("bar")),
            (OsStr::new("BAZ"), OsStr::new("quux")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let actual = parse_env(Cursor::new(input)).unwrap();
        assert_eq!(actual, expected);
    }
}
