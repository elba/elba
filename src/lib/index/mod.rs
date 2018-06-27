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
//!
//! Indices can also act like Purescript package sets, in that they can refer to git repositories.

use err::{Error, ErrorKind};
use failure::ResultExt;
use package::{version::Constraint, *};
use semver::Version;
use serde_json;
use std::collections::BTreeMap;
use std::{
    fs, io::{self, BufRead}, path::PathBuf,
};
use url::Url;

/// A dependency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct Dep {
    pub name: PackageId,
    pub req: Constraint,
}

// TODO: IndexMap<Resolution, Index> so we can select faster?
pub struct Indices(Vec<Index>);

impl Indices {
    pub fn select(&mut self, pkg: Summary) -> Result<IndexEntry, Error> {
        for ix in &mut self.0 {
            if let Ok(r) = ix.select(&pkg) {
                return Ok(r);
            }
        }

        return Err(ErrorKind::NotInIndex)?;
    }
}

// TODO: We could separate source out into a special thing for IndexEntry. But needless duplication...
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct IndexEntry {
    #[serde(flatten)]
    sum: Summary,
    dependencies: Vec<Dep>,
    yanked: bool,
    checksum: Checksum,
    location: Resolution,
}

// TODO: Dealing with where to download the Index, using the Config to get that info.
/// Struct `Index` defines a single index.
///
/// Indices must be sharded by group name.
pub struct Index {
    /// Indicates identifying information about the index
    id: Resolution,
    /// Indicates where this index is stored on-disk.
    path: PathBuf,
    // TODO: Should this hold urls too?
    /// Contains a mapping of versioned packages to checksums
    checksums: BTreeMap<Name, BTreeMap<Version, Checksum>>,
    cache: BTreeMap<Name, Vec<IndexEntry>>,
}

impl Index {
    /// Method `Index::new` creates a new empty package index directly from a Url and a local path.
    pub fn new(url: Url, path: PathBuf) -> Self {
        let id = Resolution::Index { url };
        let checksums = BTreeMap::new();
        let cache = BTreeMap::new();
        Index {
            id,
            path,
            checksums,
            cache,
        }
    }

    pub fn checksum(&mut self, pkg: PackageId, vers: &Version) -> Result<Checksum, Error> {
        let name = pkg.name();

        if let Some(s) = self.checksums.get(name).and_then(|v| v.get(vers)) {
            return Ok(s.clone());
        }

        self.entries(name)?;
        Ok(self.checksums[name][vers].clone())
    }

    pub fn entries(&mut self, name: &Name) -> Result<&Vec<IndexEntry>, Error> {
        if !self.cache.contains_key(&name) {
            let summaries = self.load_entries(name)?;
            self.cache.insert(name.clone(), summaries);
        }

        Ok(&self.cache[name])
    }

    /// Method `Index::find` searches for a package within the index, invoking a callback on its
    /// contents.
    pub fn load_entries(&mut self, name: &Name) -> Result<Vec<IndexEntry>, Error> {
        let mut res = Vec::new();
        let mut path = self.path.clone();
        path.push(name.group());
        path.push(name.name());

        self.checksums.insert(name.clone(), BTreeMap::new());

        let file = fs::File::open(path).context(ErrorKind::NotInIndex)?;
        let r = io::BufReader::new(&file);

        for line in r.lines() {
            let line = line.context(ErrorKind::InvalidIndex)?;
            let entry = self.parse_index_entry(&line)?;

            res.push(entry);
        }

        Ok(res)
    }

    // TODO: What to do with unused fields
    fn parse_index_entry(&mut self, line: &str) -> Result<IndexEntry, Error> {
        let IndexEntry {
            sum,
            dependencies,
            checksum,
            yanked,
            location,
        } = serde_json::from_str(&line).context(ErrorKind::InvalidIndex)?;

        self.checksums
            .get_mut(sum.name())
            .unwrap()
            .insert(sum.version().clone(), checksum.clone());

        Ok(IndexEntry {
            sum,
            dependencies,
            checksum,
            yanked,
            location,
        })
    }

    pub fn select(&mut self, pkg: &Summary) -> Result<IndexEntry, Error> {
        let sums = self.entries(pkg.name())?;

        for s in sums {
            if &s.sum == pkg {
                return Ok(s.clone());
            }
        }

        return Err(ErrorKind::NotInIndex)?;
    }
}
