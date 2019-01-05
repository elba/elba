//! Locking to make sure that multiple copies of `elba` don't clobber each other.
//!
//! As it is currently designed, `elba` doesn't need to lock individual files. It does, however,
//! need to lock directories to prevent other processes from using them.

use failure::{bail, format_err, Error, ResultExt};
use fs2::FileExt;
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
        fs::create_dir_all(&path).with_context(|e| {
            format_err!(
                "couldn't create dir {} while locking: {}",
                path.display(),
                e
            )
        })?;

        let lock_path = path.join(".dirlock");

        let f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&lock_path)
            .with_context(|e| {
                format_err!("couldn't open lockfile {}: {}", lock_path.display(), e)
            })?;

        if f.metadata()
            .with_context(|e| format_err!("couldn't get lockfile metadata: {}", e))?
            .len()
            != 0
        {
            bail!(
                "lockfile name conflict with existing file {}",
                lock_path.display()
            )
        }

        f.lock_exclusive().with_context(|e| {
            format_err!("couldn't lock lockfile {}: {}", lock_path.display(), e)
        })?;

        Ok(DirLock {
            path: path.to_path_buf(),
            lock_path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let f = fs::OpenOptions::new()
            .read(true)
            .create(true)
            .open(&self.lock_path);

        if let Ok(f) = f {
            let _ = f.unlock();
        }

        let _ = fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;

    use super::*;
    use std::fs;

    #[test]
    fn dirlock_simple() {
        // As long as nothing panics, we're ok.
        let tmp = tempdir::TempDir::new("elba").unwrap();
        DirLock::acquire(tmp.path()).unwrap();
    }

    #[test]
    fn dirlock_wrong_order() {
        // As long as nothing panics, we're ok.
        let tmp = tempdir::TempDir::new("elba").unwrap();
        let lock = DirLock::acquire(tmp.path()).unwrap();

        // Purposely drop in the wrong order
        drop(tmp);
        drop(lock);
    }

    #[test]
    fn dirlock_existing_err() {
        let tmp = tempdir::TempDir::new("elba").unwrap();
        fs::write(tmp.path().join(".dirlock"), b"hello world").unwrap();

        let lock = DirLock::acquire(tmp.path());

        assert!(lock.is_err());
    }
}
