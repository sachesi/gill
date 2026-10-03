//! What bash writes about itself after a command: its working directory and exported
//! environment, as NUL-separated fields.
//!
//! The layout is `CWD \0 (NAME \0 VALUE \0)* \0`. A name is never empty, so the empty
//! field at the end marks a dump that bash finished writing.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

#[derive(Debug, PartialEq, Eq)]
pub struct Dump {
    pub cwd: OsString,
    pub environ: Vec<(OsString, OsString)>,
}

impl Dump {
    /// Returns `None` for a dump that was cut short.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let body = bytes.strip_suffix(b"\0")?;
        let mut fields = body.split(|&b| b == 0);
        let cwd = fields.next()?.to_vec();
        let mut environ = Vec::new();
        loop {
            let name = fields.next()?;
            if name.is_empty() {
                break;
            }
            let value = fields.next()?;
            environ.push((
                OsString::from_vec(name.to_vec()),
                OsString::from_vec(value.to_vec()),
            ));
        }
        fields.next().is_none().then(|| Self {
            cwd: OsString::from_vec(cwd),
            environ,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_dump_keeps_values_byte_for_byte() {
        let dump = Dump::parse(b"/home/me\0A\0x y\nz\0EMPTY\0\0\0").unwrap();
        assert_eq!(dump.cwd, "/home/me");
        assert_eq!(
            dump.environ,
            [("A".into(), "x y\nz".into()), ("EMPTY".into(), "".into())]
        );
    }

    #[test]
    fn dump_without_variables_is_complete() {
        let dump = Dump::parse(b"/\0\0").unwrap();
        assert!(dump.environ.is_empty());
    }

    #[test]
    fn dump_cut_short_is_refused() {
        for bytes in [&b""[..], b"/", b"/\0", b"/\0A\0", b"/\0A\0x\0", b"/\0A\0x"] {
            assert_eq!(Dump::parse(bytes), None, "{bytes:?}");
        }
    }

    #[test]
    fn data_after_the_end_marker_is_refused() {
        assert_eq!(Dump::parse(b"/\0\0A\0"), None);
    }
}
