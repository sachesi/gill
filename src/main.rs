mod dump;
mod record;
mod run;
mod session;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use tracing_subscriber::EnvFilter;

use crate::run::Command;
use crate::session::Session;

const FISH_INIT: &str = include_str!("../shell/gill.fish");

const USAGE: &str = "\
usage: gill init fish
       gill run --session ID -c LINE
       gill run --session ID -- COMMAND [ARG...]
       gill end --session ID
";

/// Exit status for gill's own failures, kept apart from the statuses bash commands use.
const FAILURE: u8 = 125;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with_writer(io::stderr)
        .without_time()
        .init();

    match dispatch(std::env::args_os().skip(1).collect()) {
        Ok(status) => ExitCode::from(status),
        Err(err) => {
            eprintln!("gill: {err:#}");
            ExitCode::from(FAILURE)
        }
    }
}

fn dispatch(args: Vec<OsString>) -> Result<u8> {
    let mut args = args.into_iter();
    let Some(subcommand) = args.next() else {
        bail!("missing subcommand\n{USAGE}");
    };
    match subcommand.to_str() {
        Some("init") => {
            if args.next().as_deref() != Some("fish".as_ref()) || args.next().is_some() {
                bail!("only fish is supported\n{USAGE}");
            }
            io::stdout()
                .write_all(FISH_INIT.as_bytes())
                .context("writing the fish integration")?;
            Ok(0)
        }
        Some("run") => {
            let session = Session::open(&session_id(&mut args)?)?;
            let rest: Vec<OsString> = args.collect();
            let command = match rest.split_first() {
                Some((flag, [line])) if flag == "-c" => Command::Line(line),
                Some((flag, words)) if flag == "--" && !words.is_empty() => Command::Argv(words),
                _ => bail!("expected -c LINE or -- COMMAND\n{USAGE}"),
            };
            run::run(&session, &command)
        }
        Some("end") => {
            let id = session_id(&mut args)?;
            if args.next().is_some() {
                bail!("unexpected argument after the session\n{USAGE}");
            }
            Session::remove(&id)?;
            Ok(0)
        }
        Some("-h" | "--help") => {
            print!("{USAGE}");
            Ok(0)
        }
        _ => bail!(
            "unknown subcommand {}\n{USAGE}",
            subcommand.to_string_lossy()
        ),
    }
}

fn session_id(args: &mut impl Iterator<Item = OsString>) -> Result<String> {
    if args.next().as_deref() != Some("--session".as_ref()) {
        bail!("missing --session\n{USAGE}");
    }
    let id = args
        .next()
        .context("missing the session id after --session")?;
    id.into_string()
        .map_err(|id| anyhow::anyhow!("session id {} is not UTF-8", id.to_string_lossy()))
}
