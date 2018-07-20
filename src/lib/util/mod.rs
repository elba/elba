//! Utility functions.

pub mod config;
pub mod errors;
pub mod lock;
pub mod shell;

use failure::ResultExt;
use std::{fs, io::Write, path::Path};
use util::errors::Res;

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
    (|| -> Res<()> {
        let mut f = fs::File::create(path)?;
        f.write_all(contents)?;
        Ok(())
    })().context(format_err!("failed to write `{}`", path.display()))?;
    Ok(())
}
