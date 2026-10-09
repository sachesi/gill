use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::process;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use signal_hook::consts::{SIGINT, SIGQUIT};

use crate::dump::Dump;
use crate::record;
use crate::session::Session;

const PRELUDE: &str = include_str!("../shell/prelude.bash");

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
pub fn run(session: &Session, command: &Command) -> Result<u8> {
    session.clear_results()?;

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
        .arg(session.dir());
    match command {
        Command::Line(line) => bash.arg("line").arg(line),
        Command::Argv(argv) => bash.arg("argv").args(*argv),
    };
    let status = bash.status().context("starting bash")?;

    if let Some(dump) = session.take_environ()?.as_deref().and_then(Dump::parse) {
        session.keep_snapshot()?;
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
