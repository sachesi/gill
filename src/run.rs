use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use signal_hook::consts::{SIGINT, SIGQUIT};

use crate::dump::Dump;
use crate::record;
use crate::session::Session;

const PRELUDE: &str = include_str!("../shell/prelude.bash");

/// What the bash side brings across besides the command itself.
#[derive(Debug, Default)]
pub struct Imports {
    /// Source `~/.bashrc` into the bash state, once per session and again when its
    /// content changes.
    pub bashrc: bool,
    /// Variables fish takes from bash, exported or not.
    pub names: Vec<OsString>,
}

#[derive(Debug)]
pub enum Command<'a> {
    /// A command line, parsed by bash as typed.
    Line(&'a OsString),
    /// A command and its arguments, run as separate words.
    Argv(&'a [OsString]),
}

/// Runs the command in a fresh bash restored from the session's snapshot, then leaves
/// the new snapshot and the records fish applies in the session directory.
/// Returns the command's exit status.
pub fn run(session: &Session, command: &Command, imports: &Imports) -> Result<u8> {
    session.clear_results()?;
    let bashrc = if imports.bashrc {
        bashrc_to_source(session)?
    } else {
        None
    };

    // Ctrl-C and Ctrl-\ reach the whole foreground job. gill has to outlive bash to
    // keep its state, as system(3) does; bash gets the default handlers back at exec.
    // The flag is never read: installing a handler is what keeps gill running.
    for signal in [SIGINT, SIGQUIT] {
        signal_hook::flag::register(signal, Arc::new(AtomicBool::new(false)))
            .context("installing the signal handlers")?;
    }

    let mut bash = process::Command::new("bash");
    bash.args(["--noprofile", "--norc", "-c", PRELUDE, "bash"])
        .arg(session.snapshot())
        .arg(session.dir())
        .arg(
            bashrc
                .as_ref()
                .map_or(OsStr::new(""), |(path, _)| path.as_os_str()),
        )
        .arg(imports.names.join(OsStr::new(" ")));
    match command {
        Command::Line(line) => bash.arg("line").arg(line),
        Command::Argv(argv) => bash.arg("argv").args(*argv),
    };
    let status = bash.status().context("starting bash")?;

    if let Some(dump) = session.take_environ()?.as_deref().and_then(Dump::parse) {
        session.keep_snapshot()?;
        if let Some((_, content)) = &bashrc {
            session.write_seen_bashrc(content)?;
        }
        let records = record::diff(std::env::vars_os(), dump);
        session.write_records(&record::encode(&records))?;
    } else {
        session.discard_snapshot()?;
        tracing::warn!(
            "bash exited without saving its state (exec, or an EXIT trap of its own); \
             the state from before this command is kept"
        );
    }

    // A signal is reported the way shells do, as 128 + its number.
    Ok(status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}

/// The `~/.bashrc` to source before this command, with its content, when the file
/// exists and differs from what this session last sourced.
fn bashrc_to_source(session: &Session) -> Result<Option<(PathBuf, Vec<u8>)>> {
    let Some(home) = std::env::var_os("HOME") else {
        return Ok(None);
    };
    let path = PathBuf::from(home).join(".bashrc");
    let content = match fs::read(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };
    if session.seen_bashrc()?.as_deref() == Some(content.as_slice()) {
        return Ok(None);
    }
    Ok(Some((path, content)))
}
