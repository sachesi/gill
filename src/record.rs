//! Changes fish applies after a bash command, encoded as NUL-separated
//! `OP \0 NAME \0 VALUE \0` triples that one fixed fish function reads as data.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

use crate::dump::Dump;

/// Variables bash or fish manage themselves. Copying them across would fight the shell
/// that owns them (`SHLVL` would grow by one with every command).
const UNSYNCED: &[&str] = &[
    "_",
    "CMD_DURATION",
    "COLUMNS",
    "EUID",
    "IFS",
    "LINES",
    "OLDPWD",
    "PWD",
    "SHLVL",
    "argv",
    "history",
    "hostname",
    "last_pid",
    "pipestatus",
    "status",
    "status_generation",
    "umask",
    "version",
];

const UNSYNCED_PREFIXES: &[&str] = &["fish_", "FISH_", "__fish", "__gill"];

#[derive(Debug, PartialEq, Eq)]
pub enum Record {
    Set(OsString, OsString),
    Erase(OsString),
    Cd(OsString),
}

/// What fish must change so its exported environment and directory match bash's.
pub fn diff(before: impl IntoIterator<Item = (OsString, OsString)>, after: Dump) -> Vec<Record> {
    let before: BTreeMap<OsString, OsString> = before.into_iter().collect();
    let after_environ: BTreeMap<OsString, OsString> = after.environ.into_iter().collect();
    let mut records = Vec::new();

    if before.get(OsStr::new("PWD")) != Some(&after.cwd) {
        records.push(Record::Cd(after.cwd));
    }
    for (name, value) in &after_environ {
        if is_synced(name) && before.get(name) != Some(value) {
            records.push(Record::Set(name.clone(), value.clone()));
        }
    }
    for name in before.keys() {
        if is_synced(name) && !after_environ.contains_key(name) {
            records.push(Record::Erase(name.clone()));
        }
    }
    records
}

pub fn encode(records: &[Record]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        let (op, name, value): (&str, &OsStr, &OsStr) = match record {
            Record::Set(name, value) => ("set", name, value),
            Record::Erase(name) => ("erase", name, OsStr::new("")),
            Record::Cd(path) => ("cd", OsStr::new(""), path),
        };
        for field in [OsStr::new(op), name, value] {
            bytes.extend_from_slice(field.as_bytes());
            bytes.push(0);
        }
    }
    bytes
}

/// Whether fish may set or erase a variable of this name: a valid fish variable name
/// that neither shell manages itself.
fn is_synced(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid
        && !UNSYNCED.contains(&name)
        && !UNSYNCED_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environ(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|&(name, value)| (name.into(), value.into()))
            .collect()
    }

    fn dump(cwd: &str, pairs: &[(&str, &str)]) -> Dump {
        Dump {
            cwd: cwd.into(),
            environ: environ(pairs),
        }
    }

    #[test]
    fn unchanged_environment_needs_no_records() {
        let before = environ(&[("PWD", "/a"), ("HOME", "/h")]);
        assert_eq!(
            diff(before, dump("/a", &[("PWD", "/a"), ("HOME", "/h")])),
            []
        );
    }

    #[test]
    fn changed_added_and_removed_variables_become_records() {
        let before = environ(&[("PWD", "/a"), ("KEEP", "1"), ("GONE", "x"), ("EDIT", "old")]);
        let after = dump(
            "/b",
            &[("PWD", "/b"), ("KEEP", "1"), ("EDIT", "new"), ("ADD", "y")],
        );
        assert_eq!(
            diff(before, after),
            [
                Record::Cd("/b".into()),
                Record::Set("ADD".into(), "y".into()),
                Record::Set("EDIT".into(), "new".into()),
                Record::Erase("GONE".into()),
            ]
        );
    }

    #[test]
    fn shell_managed_and_invalid_names_are_never_synced() {
        let before = environ(&[("PWD", "/a"), ("SHLVL", "1"), ("fish_greeting", "hi")]);
        let after = dump(
            "/a",
            &[
                ("SHLVL", "2"),
                ("_", "/usr/bin/bash"),
                ("BASH_FUNC_f%%", "() { :; }"),
                ("__gill_line", "x"),
                ("1X", "x"),
            ],
        );
        assert_eq!(diff(before, after), []);
    }

    #[test]
    fn records_are_nul_separated_triples() {
        let records = [
            Record::Cd("/tmp".into()),
            Record::Set("A".into(), "x\ny'".into()),
            Record::Erase("B".into()),
        ];
        assert_eq!(encode(&records), b"cd\0\0/tmp\0set\0A\0x\ny'\0erase\0B\0\0");
    }
}
