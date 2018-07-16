//! An index from which information/metadata about available packages is obtained.
//!
//! ## Design
//! Indices provide metadata about available packages from a particular source. Multiple indices
//! can be specified at once, and a default index can be specified in configuration for packages
//! with version constraints but no explicitly specified index. By default, the official index is
//! set to be the default.
//!
//! The packages that the index offers must have a direct source: they cannot point to other
//! registries. Because the package doesn't necessarily need to be a tarball stored somewhere,
//! indices can serve to "curate" packages from disparate repositories and other sources (think
//! Purescript package sets). The dependencies of a package in an index must be located either in
//! the same index or a dependent index of the current index (as specified in the index's config).
//!
//! Tarballs are the only source which can contain a checksum, by nature of the way they're
//! constructed internally.
//!
//! A package can only be published to the official index if it only depends on packages in the
//! official index.
//!
//! ## Prior art
//! This design follows closely with that of Cargo's, specifically with their RFC enabling
//! [unofficial registries](https://github.com/rust-lang/rfcs/blob/master/text/2141-alternative-registries.md).

mod config;

use self::config::IndexConfig;
use failure::ResultExt;
use indexmap::IndexMap;
use package::{
    resolution::{DirectRes, IndexRes, Resolution},
    version::Constraint,
    *,
};
use semver::Version;
use serde_json;
use std::{
    fs,
    io::{self, prelude::*, BufReader},
    str::FromStr,
};
use url::Url;
use util::{
    err::{Error, ErrorKind},
    lock::DirLock,
};

/// A dependency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Dep {
    pub name: Name,
    // TODO: Ideally, instead of having the index be an IndexRes, we just let it be a String
    // corresponding to a key in the Index's config which points to an actual IndexRes.
    pub index: IndexRes,
    pub req: Constraint,
}

#[derive(Debug)]
pub struct Indices {
    /// The indices being used.
    ///
    /// It is assumed that all dependent indices have been resolved, and that this mapping contains
    /// every index mentioned or depended on.
    indices: IndexMap<IndexRes, Index>,
    cache: IndexMap<PackageId, IndexMap<Version, IndexEntry>>,
}

impl Indices {
    pub fn new(indices: Vec<Index>) -> Self {
        let indices = indices.into_iter().map(|i| (i.id.clone(), i)).collect();
        let cache = indexmap!();

        Indices { indices, cache }
    }

    pub fn select(&mut self, pkg: &Summary) -> Result<&IndexEntry, Error> {
        let entry = self
            .entries(pkg.id())?
            .get(pkg.version())
            .ok_or_else(|| ErrorKind::PackageNotFound)?;

        Ok(entry)
    }

    pub fn count_versions(&self, pkg: &PackageId) -> usize {
        match self.cache.get(pkg) {
            Some(m) => m.len(),
            None => 0,
        }
    }

    pub fn entries(&mut self, pkg: &PackageId) -> Result<&IndexMap<Version, IndexEntry>, Error> {
        if self.cache.contains_key(pkg) {
            return Ok(&self.cache[pkg]);
        }

        let res = pkg.resolution();
        if let Resolution::Index(ir) = res {
            let ix = self.indices.get(ir);

            if let Some(ix) = ix {
                let mut v = ix.entries(pkg.name())?;
                v.sort_keys();
                self.cache.insert(pkg.clone(), v);
                Ok(&self.cache[pkg])
            } else {
                Err(Error::from(ErrorKind::PackageNotFound))
            }
        } else {
            Err(Error::from(ErrorKind::PackageNotFound))
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IndexEntry {
    pub name: Name,
    pub version: Version,
    pub dependencies: Vec<Dep>,
    pub yanked: bool,
    pub location: DirectRes,
}

// TODO: Dealing with where to download the Index, using the Config to get that info.
// TODO: user-friendly index names? (this decouples the index from its url; we'd need a function to
// turn these user-friendly names into explicitly ones)
/// Struct `Index` defines a single index.
///
/// Indices must be sharded by group name.
#[derive(Debug)]
pub struct Index {
    /// Indicates identifying information about the index
    pub id: IndexRes,
    /// Indicates where this index is stored on-disk.
    pub path: DirLock,
    /// The configuration of this index.
    pub config: IndexConfig,
}

impl Index {
    /// Creates a new empty package index directly from a Url and a local path.
    pub fn from_disk(url: Url, path: DirLock) -> Result<Self, Error> {
        let id = IndexRes { url };
        let pn = path.path().join("index.toml");
        let file = fs::File::open(pn).context(ErrorKind::InvalidIndex)?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context(ErrorKind::InvalidIndex)?;
        let config = IndexConfig::from_str(&contents).context(ErrorKind::InvalidIndex)?;

        Ok(Index { id, path, config })
    }

    pub fn entries(&self, name: &Name) -> Result<IndexMap<Version, IndexEntry>, Error> {
        let mut res = indexmap!();
        let path = self.path.path().join(name.as_str());
        let file = fs::File::open(path).context(ErrorKind::PackageNotFound)?;
        let r = io::BufReader::new(&file);

        for line in r.lines() {
            let line = line.context(ErrorKind::InvalidIndex)?;
            let entry: IndexEntry = serde_json::from_str(&line).context(ErrorKind::InvalidIndex)?;

            res.insert(entry.version.clone(), entry);
        }

        Ok(res)
    }
}
