mod dump;
mod record;
mod run;
mod session;

use std::env::VarError;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use tracing_subscriber::EnvFilter;

use crate::run::{Command, Imports};
use crate::session::Session;

const FISH_INIT: &str = include_str!("../shell/gill.fish");

const USAGE: &str = "\
usage: gill init fish [--bashrc]
       gill run [--bashrc] --session ID -c LINE
       gill run [--bashrc] --session ID -- COMMAND [ARG...]
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
            let bashrc = match args.as_slice() {
                [fish] if fish == "fish" => false,
                [fish, flag] if fish == "fish" && flag == "--bashrc" => true,
                _ => bail!("only fish is supported, with an optional --bashrc\n{USAGE}"),
            };
            let mut stdout = io::stdout();
            if bashrc {
                stdout
                    .write_all(b"set -g __gill_run_options --bashrc\n")
                    .context("writing the fish integration")?;
            }
            stdout
                .write_all(FISH_INIT.as_bytes())
                .context("writing the fish integration")?;
            Ok(0)
        }
        Some("run") => {
            let bashrc_flag = args.as_slice().first().is_some_and(|arg| arg == "--bashrc");
            if bashrc_flag {
                args.next();
            }
            let session = Session::open(&session_id(&mut args)?)?;
            let rest: Vec<OsString> = args.collect();
            let command = match rest.split_first() {
                Some((flag, [line])) if flag == "-c" => Command::Line(line),
                Some((flag, words)) if flag == "--" && !words.is_empty() => Command::Argv(words),
                _ => bail!("expected -c LINE or -- COMMAND\n{USAGE}"),
            };
            run::run(&session, &command, &imports(bashrc_flag)?)
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

/// Imports from the flag and the environment: `GILL_BASHRC=1` turns the bashrc import
/// on, and `GILL_IMPORT_VARS` names the variables fish takes from bash.
fn imports(bashrc_flag: bool) -> Result<Imports> {
    let names = match std::env::var("GILL_IMPORT_VARS") {
        Ok(list) => record::import_names(&list)?,
        Err(VarError::NotPresent) => Vec::new(),
        Err(err) => bail!("GILL_IMPORT_VARS: {err}"),
    };
    Ok(Imports {
        bashrc: bashrc_flag || std::env::var_os("GILL_BASHRC").is_some_and(|value| value == "1"),
        names,
    })
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
