//! Utility functions.

pub mod config;
pub mod errors;
pub mod graph;
pub mod lock;
pub mod shell;

use failure::ResultExt;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fs, io::Write, path::Path};
use std::{
    path::{Component, PathBuf},
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
        .filter(|x| x.is_ok() && valid_file(x.as_ref().unwrap()));
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

pub fn clear_dir(dir: &Path) -> Res<()> {
    fs::remove_dir_all(dir)?;
    fs::create_dir(dir)?;
    Ok(())
}

fn valid_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
}
