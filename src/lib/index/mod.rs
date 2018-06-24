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

use failure::ResultExt;
use semver::Version;
use serde_json;
use std::collections::BTreeMap;
use std::{
    fs, io::{self, BufRead}, path::PathBuf,
};
use url::Url;
use url_serde;

use err::{Error, ErrorKind};
use package::*;

pub type Indices = Vec<Index>;

// TODO: We could separate source out into a special thing for IndexEntry. But needless duplication...
#[derive(Debug, Deserialize, Serialize)]
struct IndexEntry {
    name: Name,
    version: Version,
    dependencies: Vec<Dep>,
    yanked: bool,
    checksum: Checksum,
    #[serde(flatten)]
    source: Resolution, // TODO: Recursive sources. One registry points to another points to...
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
    cache: BTreeMap<Name, Vec<Summary<Dep>>>,
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

    pub fn checksum(&mut self, pkg: PackageId) -> Result<Checksum, Error> {
        let name = pkg.name();
        let vers = pkg.version();

        if let Some(s) = self.checksums.get(name).and_then(|v| v.get(vers)) {
            return Ok(s.clone());
        }

        self.summaries(name)?;
        Ok(self.checksums[name][vers].clone())
    }

    pub fn summaries(&mut self, name: &Name) -> Result<&Vec<Summary<Dep>>, Error> {
        if !self.cache.contains_key(&name) {
            let summaries = self.load_summaries(name)?;
            self.cache.insert(name.clone(), summaries);
        }

        Ok(&self.cache[name])
    }

    /// Method `Index::find` searches for a package within the index, invoking a callback on its
    /// contents.
    pub fn load_summaries(&mut self, name: &Name) -> Result<Vec<Summary<Dep>>, Error> {
        let mut res = Vec::new();
        let mut path = self.path.clone();
        path.push(name.group());
        path.push(name.name());

        self.checksums.insert(name.clone(), BTreeMap::new());

        let file = fs::File::open(path).context(ErrorKind::NotInIndex)?;
        let r = io::BufReader::new(&file);

        for line in r.lines() {
            let line = line.context(ErrorKind::InvalidIndex)?;
            let sum: Summary<Dep> = self.parse_index_entry(&line)?;

            res.push(sum);
        }

        Ok(res)
    }

    // TODO: What to do with unused fields
    fn parse_index_entry(&mut self, line: &str) -> Result<Summary<Dep>, Error> {
        let IndexEntry {
            name,
            version,
            dependencies,
            checksum,
            yanked: _yanked,
            source: _source,
        } = serde_json::from_str(&line).context(ErrorKind::InvalidIndex)?;

        let pkg = PackageId::new(name, version.clone(), self.id.clone());
        let sum = Summary::new(pkg, checksum.clone(), dependencies);

        self.checksums
            .get_mut(sum.id().name())
            .unwrap()
            .insert(version, checksum);

        Ok(sum)
    }

    /// Method `Index::retrieve` returns a Url from which you can download or find a package.
    pub fn retrieve(&self, pkg: &PackageId) -> Result<Url, Error> {
        unimplemented!()
    }
}
