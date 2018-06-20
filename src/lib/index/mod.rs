//! Module `index` defines the format of and the interface for a package index.
//!
//! First, some definitions:
//! *index* = a listing of package metadata, including versions, dependencies, and source urls.
//! *registry* = a place where package archives are hosted.
//!
//! Indices are the main way to get information about packages. Indices do not correspond one-to-
//! one with registries; they can refer to packages in a registry, or to git repositories, or to
//! local directories. To which they are referring to is disambiguated from the Url schema.
//!
//! Multiple indices can be used at the same time. Indices that are listed first have "higher
//! priority," and their entries override subsequent indices' entries.
//!
//! Only packages which rely solely on the official index and depend on packages in the official
//! registry can be published to the official registry.
//! This can be manually checked for by unofficial registries.

use failure::ResultExt;
use semver::Version;
use serde_json;
use std::collections::BTreeMap;
use std::{
    fs, io::{self, BufRead}, path::PathBuf,
};
use toml;
use url::Url;

use err::{Error, ErrorKind};
use package::*;

pub struct IndexSet {
    /// Indicates identifying information about the index
    ixs: Vec<Index>,
    /// Contains a mapping of versioned packages to checksums
    hashes: BTreeMap<Name, BTreeMap<Version, Checksum>>,
    cache: BTreeMap<Name, BTreeMap<Version, (Index, Summary<Dep>)>>,
}

impl IndexSet {
    pub fn query(&mut self, pkg: &PackageId, f: &mut FnMut(&Summary<Dep>)) -> Result<(), Error> {
        if !self.cache.contains_key(pkg.name()) {
            self.cache.insert(pkg.name().clone(), BTreeMap::new());
            for ix in &mut self.ixs {
                ix.query(pkg, &mut |e| {
                    unimplemented!()
                })?;
            }
        }

        Ok(())
    }
}

/// Struct `Index` defines a single index.
///
/// Indices must be sharded by group name.
pub struct Index {
    /// Indicates identifying information about the index
    id: Source,
    /// Indicates where this index is stored on-disk.
    path: PathBuf,
    /// Contains a mapping of versioned packages to checksums
    hashes: BTreeMap<Name, BTreeMap<Version, Checksum>>,
    cache: BTreeMap<Name, Vec<Summary<Dep>>>,
}

impl Index {
    /// Method `Index::new` creates a new empty package index directly from a Url and a local path.
    pub fn new(&self, url: Url, path: PathBuf) -> Self {
        let id = Source::Index { url };
        let hashes = BTreeMap::new();
        let cache = BTreeMap::new();
        Index {
            id,
            path,
            hashes,
            cache,
        }
    }

    pub fn load(&mut self, sum: Summary<Dep>) {
        let name = sum.id().name().clone();
        if !self.cache.contains_key(&name) {
            let vers = sum.id().version().clone();
            let mut v = BTreeMap::new();
            v.insert(vers, sum.checksum().clone());
            self.hashes.insert(name.clone(), v);

            self.cache.insert(name, vec![sum]);
        }
    }

    /// Method `Index::find` searches for a package within the index, invoking a callback on its
    /// contents.
    pub fn query(&mut self, pkg: &PackageId, f: &mut FnMut(&Summary<Dep>)) -> Result<(), Error> {
        if !self.cache.contains_key(pkg.name()) {
            let mut path = self.path.clone();
            path.push(pkg.name().group());
            path.push(pkg.name().as_str());

            let file = fs::File::open(path).context(ErrorKind::InvalidIndex)?;
            let r = io::BufReader::new(&file);

            for line in r.lines() {
                let line = line.context(ErrorKind::InvalidIndex)?;
                let sum: Summary<Dep> =
                    serde_json::from_str(&line).context(ErrorKind::InvalidIndex)?;

                self.load(sum);
            }
        }

        for p in self.cache.get(pkg.name()) {
            for s in p {
                f(s);
            }
        }

        Ok(())
    }
}
