//! Locking to make sure that multiple copies of `elba` don't clobber each other.

use std::{fs, io, path::{Path, PathBuf}};

/// A lock on a directory. This just generates a sibling file to the directory which indicates that
/// the directory is locked. 
pub struct DirLock {
    path: PathBuf,
    lock_path: PathBuf,
}

// TODO: impl Drop?
impl DirLock {
    pub fn acquire<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = fs::canonicalize(path)?;
        let lock_path = { let mut p = path.clone(); p.set_extension("lock"); p };
        fs::OpenOptions::new().write(true).create_new(true).open(&lock_path).map(|_| DirLock { path, lock_path })
    }

    pub fn release(self) -> io::Result<()> {
        fs::remove_file(self.lock_path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}