//! Locking to make sure that multiple copies of `elba` don't clobber each other.
//!
//! As it is currently designed, `elba` doesn't need to lock individual files. It does, however,
//! need to lock directories to prevent other processes from using them.

use super::errors::ErrorKind;
use failure::{Error, ResultExt};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A lock on a directory. This just generates a sibling file to the directory which indicates that
/// the directory is locked.
#[derive(Debug, PartialEq, Eq)]
pub struct DirLock {
    path: PathBuf,
    lock_path: PathBuf,
}

impl DirLock {
    pub fn acquire(path: &Path) -> Result<Self, Error> {
        // Note! canonicalize will error if the path does not already exist.
        // let path = fs::canonicalize(path).context(ErrorKind::Locked)?;

        let lock_path = { path.with_extension("lock") };

        let res = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map(|_| DirLock {
                path: path.to_path_buf(),
                lock_path,
            })
            .context(ErrorKind::Locked)?;

        Ok(res)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}
