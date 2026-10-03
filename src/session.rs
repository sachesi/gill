//! The private directory holding one fish session's bash state, under
//! `$XDG_RUNTIME_DIR/gill/<id>/`.

use std::ffi::OsStr;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

const SNAPSHOT: &str = "snapshot";
const NEXT_SNAPSHOT: &str = "snapshot.next";
const ENVIRON: &str = "environ.next";
const RECORDS: &str = "records";

#[derive(Debug)]
pub struct Session {
    dir: PathBuf,
}

impl Session {
    /// Opens the session's directory, creating it on first use.
    pub fn open(id: &str) -> Result<Self> {
        let (base, owner) = base_dir()?;
        let dir = base.join(checked_id(id)?);
        private_dir(&dir, owner)?;
        Ok(Self { dir })
    }

    pub fn remove(id: &str) -> Result<()> {
        let (base, _) = base_dir()?;
        let dir = base.join(checked_id(id)?);
        match fs::remove_dir_all(&dir) {
            Err(err) if err.kind() != ErrorKind::NotFound => {
                Err(err).with_context(|| format!("removing {}", dir.display()))
            }
            _ => Ok(()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The bash state the next command starts from.
    pub fn snapshot(&self) -> PathBuf {
        self.dir.join(SNAPSHOT)
    }

    /// Removes what an earlier command left behind, so nothing from it is read as this
    /// command's result.
    pub fn clear_results(&self) -> Result<()> {
        for name in [NEXT_SNAPSHOT, ENVIRON, RECORDS] {
            remove_if_present(&self.dir.join(name))?;
        }
        Ok(())
    }

    /// The environment bash wrote when the command finished, if it got that far.
    pub fn take_environ(&self) -> Result<Option<Vec<u8>>> {
        let path = self.dir.join(ENVIRON);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        remove_if_present(&path)?;
        Ok(Some(bytes))
    }

    /// Makes the snapshot bash wrote with the environment the one the next command
    /// restores.
    pub fn keep_snapshot(&self) -> Result<()> {
        let next = self.dir.join(NEXT_SNAPSHOT);
        fs::rename(&next, self.snapshot()).with_context(|| format!("renaming {}", next.display()))
    }

    pub fn discard_snapshot(&self) -> Result<()> {
        remove_if_present(&self.dir.join(NEXT_SNAPSHOT))
    }

    pub fn write_records(&self, bytes: &[u8]) -> Result<()> {
        let path = self.dir.join(RECORDS);
        // create_new refuses an existing file or symlink, so this always writes a fresh
        // file that only the owner can read.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut file| file.write_all(bytes))
            .with_context(|| format!("writing {}", path.display()))
    }
}

/// `$XDG_RUNTIME_DIR/gill` and the uid that owns the runtime directory.
fn base_dir() -> Result<(PathBuf, u32)> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .context("XDG_RUNTIME_DIR is not set")?;
    ensure!(
        runtime.is_absolute(),
        "XDG_RUNTIME_DIR {} is not an absolute path",
        runtime.display()
    );
    let meta = fs::metadata(&runtime)
        .with_context(|| format!("reading XDG_RUNTIME_DIR {}", runtime.display()))?;
    // Only the owner can create entries in a 0700 directory, so everything below it
    // that the same uid owns was made by that user.
    if !meta.is_dir() || meta.mode() & 0o077 != 0 {
        bail!(
            "XDG_RUNTIME_DIR {} is not a directory only its owner can access",
            runtime.display()
        );
    }
    let base = runtime.join("gill");
    private_dir(&base, meta.uid())?;
    Ok((base, meta.uid()))
}

fn private_dir(path: &Path, owner: u32) -> Result<()> {
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => return Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err).with_context(|| format!("creating {}", path.display())),
    }
    let meta = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if !meta.is_dir() || meta.uid() != owner || meta.mode() & 0o077 != 0 {
        bail!(
            "{} is not a private directory owned by uid {owner}",
            path.display()
        );
    }
    Ok(())
}

/// Session ids come from fish as `<pid>-<random>`; anything else could name a path
/// outside the session directory.
fn checked_id(id: &str) -> Result<&OsStr> {
    ensure!(
        !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_digit() || b == b'-'),
        "invalid session id {id:?}"
    );
    Ok(OsStr::new(id))
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != ErrorKind::NotFound => {
            Err(err).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}
