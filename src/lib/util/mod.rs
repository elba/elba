//! Utility functions.

pub mod config;
pub mod errors;
pub mod git;
pub mod graph;
pub mod lock;
pub mod shell;

use failure::ResultExt;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fs, path::Path};
use std::{
    path::{Component, PathBuf},
    process::Output,
    str::FromStr,
};
use util::errors::{Error, Res};
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubPath(pub PathBuf);

impl SubPath {
    pub fn is_subpath(p: &Path) -> bool {
        p.is_relative() && p.components().all(|x| x != Component::ParentDir)
    }

    pub fn from_path(p: &Path) -> Res<Self> {
        if SubPath::is_subpath(&p) {
            Ok(SubPath(p.to_path_buf()))
        } else {
            bail!("p {} isn't a strict subdirectory", p.display())
        }
    }
}

impl FromStr for SubPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = PathBuf::from(s);
        SubPath::from_path(&path)
    }
}

impl Serialize for SubPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.to_string_lossy().as_ref())
    }
}

impl<'de> Deserialize<'de> for SubPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

pub fn copy_dir(from: &Path, to: &Path) -> Res<()> {
    let walker = WalkDir::new(from)
        .into_iter()
        .filter_entry(|x| x.path() != to)
        .filter(|x| x.is_ok() && valid_file(x.as_ref().unwrap()));
    for entry in walker {
        let entry = entry.unwrap();
        let to_p = to.join(entry.path().strip_prefix(from).unwrap());
        // Make sure that the file exists before we try copying
        fs::create_dir_all(to_p.parent().unwrap())?;
        fs::File::create(&to_p).context(format_err!("couldn't create file {}", to_p.display()))?;
        let _ = fs::copy(entry.path(), &to_p).with_context(|e| {
            format_err!(
                "couldn't copy {} to {}:\n{}",
                entry.path().display(),
                to_p.display(),
                e
            )
        })?;
    }

    Ok(())
}

pub fn clear_dir(dir: &Path) -> Res<()> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    Ok(())
}

fn valid_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
}

pub fn generate_ipkg(name: &str, src_dir: &str, opts: &str, mods: &str) -> String {
    format!(
        r#"package {}
sourcedir = {}
opts = "{}"
modules = {}
    "#,
        name, src_dir, opts, mods
    )
}

pub fn fmt_output(c: &Output) -> String {
    let mut res = String::new();
    if !c.stderr.is_empty() {
        res.push_str(format!("[stderr]\n{}\n", String::from_utf8_lossy(&c.stderr)).as_ref());
    }
    if !c.stdout.is_empty() {
        if !c.stderr.is_empty() {
            res.push_str("[stdout]\n");
        }
        res.push_str(format!("{}\n", String::from_utf8_lossy(&c.stdout)).as_ref());
    }
    // Remove the ending newline if it exists
    res.pop();
    res
}
