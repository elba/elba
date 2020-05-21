//! Utility functions.

pub mod config;
pub mod error;
pub mod git;
pub mod graph;
pub mod lock;
pub mod parser;
pub mod read2;
pub mod shell;

pub use crate::util::read2::read2;

use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Output,
    str::FromStr,
};

use failure::{bail, format_err, ResultExt};
use itertools::Itertools;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use walkdir::{DirEntry, WalkDir};

use crate::util::error::Result;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubPath(pub PathBuf);

impl SubPath {
    pub fn is_subpath(p: &Path) -> bool {
        p.is_relative() && p.components().all(|x| x != Component::ParentDir)
    }

    pub fn from_path(p: &Path) -> Result<Self> {
        if SubPath::is_subpath(&p) {
            Ok(SubPath(p.to_path_buf()))
        } else {
            bail!("p {} isn't a strict subdirectory", p.display())
        }
    }
}

impl FromStr for SubPath {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        let path = PathBuf::from(s);
        SubPath::from_path(&path)
    }
}

impl Serialize for SubPath {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.to_string_lossy().as_ref())
    }
}

impl<'de> Deserialize<'de> for SubPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

pub fn copy_dir_iter(walker: impl Iterator<Item = DirEntry>, from: &Path, to: &Path) -> Result<()> {
    for entry in walker {
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

pub fn copy_dir(from: &Path, to: &Path, gitless: bool) -> Result<()> {
    let walker = WalkDir::new(from)
        .follow_links(true)
        .into_iter()
        .filter_entry(|x| x.path() != to && (!gitless || x.file_name() != ".git"))
        .filter_map(|x| {
            x.ok()
                .and_then(|x| if valid_file(&x) { Some(x) } else { None })
        });

    copy_dir_iter(walker, from, to)
}

pub fn clear_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        remove_dir_all::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    Ok(())
}

pub fn valid_file(entry: &DirEntry) -> bool {
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

pub fn fmt_multiple(c: &shell::OutputGroup) -> String {
    c.0.iter().map(|x| fmt_output(&x)).join("\n")
}

pub fn fmt_output(c: &Output) -> String {
    let mut res = String::new();
    if !c.stderr.is_empty() {
        if !c.stdout.is_empty() {
            res.push_str("--- stdout\n");
        }
        res.push_str(format!("{}\n", String::from_utf8_lossy(&c.stderr)).as_ref());
        res.push_str(format!("--- stderr\n{}\n", String::from_utf8_lossy(&c.stderr)).as_ref());
    }
    if !c.stdout.is_empty() {
        if !c.stderr.is_empty() {
            res.push_str("--- stdout\n");
        }
        res.push_str(format!("{}\n", String::from_utf8_lossy(&c.stdout)).as_ref());
    }
    // Remove the ending newline if it exists
    res.pop();
    res
}
