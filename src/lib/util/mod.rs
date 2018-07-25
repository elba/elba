//! Utility functions.

pub mod config;
pub mod errors;
pub mod graph;
pub mod lock;
pub mod shell;

use failure::ResultExt;
use std::{fs, io::Write, path::Path};
use util::errors::Res;
use walkdir::{DirEntry, WalkDir};

/// Turns an SHA2 hash into a nice hexified string.
pub fn hexify_hash(hash: &[u8]) -> String {
    let mut s = String::new();
    for byte in hash {
        let p = format!("{:02x}", byte);
        s.push_str(&p);
    }
    s
}

// TODO: create_dir_all too?
pub fn write(path: &Path, contents: &[u8]) -> Res<()> {
    let f: Res<()> = {
        let mut f = fs::File::create(path)?;
        f.write_all(contents)?;
        Ok(())
    };

    f.context(format_err!("failed to write `{}`", path.display()))?;

    Ok(())
}

pub fn copy_dir(from: &Path, to: &Path) -> Res<()> {
    let walker = WalkDir::new(from)
        .into_iter()
        .filter_entry(|e| valid_file(e));
    for entry in walker {
        let entry = entry.unwrap();
        let to_p = to.join(entry.path().strip_prefix(from).unwrap());
        let _ = fs::copy(entry.path(), &to_p).context(format_err!(
            "couldn't copy {} to {}",
            entry.path().display(),
            to_p.display()
        ))?;
    }

    Ok(())
}

fn valid_file(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with('.'))
        .unwrap_or(false) && entry.file_type().is_file()
}
