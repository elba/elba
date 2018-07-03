//! An index from which information/metadata about available packages is obtained.
//!
//! ## Design
//! Indices provide metadata about available packages from a particular source. Multiple indices
//! can be specified at once, but packages will always default to using the index with the highest
//! priority. If a user wants to use a package from a different registry, they have to explicitly specify
//! this from their manifest. By default, the official index has the highest priority, and so the
//! official index will be assumed to be the index where the package is located. All dependencies
//! of a package listed in an index must have a source: either a direct source (i.e. a git repo or
//! tarball or file directory), or a url to another index.
//!
//! The packages that the index offers must have a direct source: they cannot point to other
//! registries. Because the package doesn't necessarily need to be a tarball stored somewhere,
//! indices can serve to "curate" packages from disparate repositories and other sources (think
//! Purescript package sets). It is assumed that the id of every package that an index offers
//! is set to that index.
//!
//! A package can only be published to the official index if it only depends on packages in the
//! official index.
//!
//! ## Prior art
//! This design follows closely with that of Cargo's, specifically with their RFC for using
//! [unofficial registries](https://github.com/rust-lang/rfcs/blob/master/text/2141-alternative-registries.md).

// TODO: If we allow git repositories, that negates the entire purpose of having an index. Now
//       if you want to find the dependencies of that git repo, you have to clone the entire package
//       and now you're back where you started. If you require that dependencies only point to
//       indices, you have to check that the Manifest == the Index (counter: you derive index values
//       from the manifest), and the index has to take on all of the dependencies of the git package...
//       Plus if ppl wanna use git repos they can just do that in their manifests.
//       Registries shouldn't deal with that.
//
//       Counter-argument: you can check before adding a direct package to an index that it doesn't
//       depend on other direct packages so that an index will always have its metadata.
//
// TODO: Can we still have the local packages available be an index? It'd be the lowest priority
//       one I guess (airplane mode moves it to highest?)
//
// TODO: Patching

mod config;

use self::config::IndexConfig;
use err::{Error, ErrorKind};
use failure::ResultExt;
use indexmap::IndexMap;
use package::{manifest::Manifest, version::Constraint, *};
use semver::Version;
use serde_json;
use std::{
    fs,
    io::{self, prelude::*, BufReader},
    path::PathBuf,
    str::FromStr,
};
use url::Url;

/// A dependency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Dep {
    pub name: Name,
    pub index: IndexRes,
    pub req: Constraint,
}

#[derive(Clone, Debug)]
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
    #[serde(flatten)]
    pub sum: Summary,
    pub dependencies: Vec<Dep>,
    pub yanked: bool,
    pub checksum: Checksum,
    pub location: DirectRes,
}

// TODO: The local packages available are an index.
// TODO: Are Lockfiles an index?
// TODO: Dealing with where to download the Index, using the Config to get that info.
/// Struct `Index` defines a single index.
///
/// Indices must be sharded by group name.
#[derive(Clone, Debug)]
pub struct Index {
    /// Indicates identifying information about the index
    pub id: IndexRes,
    /// Indicates where this index is stored on-disk.
    pub path: PathBuf,
    /// The configuration of this index.
    pub config: IndexConfig,
}

impl Index {
    /// Creates a new empty package index directly from a Url and a local path.
    pub fn from_local(url: Url, path: PathBuf) -> Result<Self, Error> {
        let id = IndexRes { url };
        let mut pn = path.clone();
        pn.push("index.toml");
        let file = fs::File::open(pn).context(ErrorKind::InvalidIndex)?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context(ErrorKind::InvalidIndex)?;
        let config = IndexConfig::from_str(&contents).context(ErrorKind::InvalidIndex)?;

        Ok(Index { id, path, config })
    }

    pub fn add(&self, manifest: &Manifest) {
        unimplemented!()
    }

    pub fn entries(&self, name: &Name) -> Result<IndexMap<Version, IndexEntry>, Error> {
        let mut res = indexmap!();
        let mut path = self.path.clone();
        path.push(name.group());
        path.push(name.name());

        let file = fs::File::open(path).context(ErrorKind::PackageNotFound)?;
        let r = io::BufReader::new(&file);

        for line in r.lines() {
            let line = line.context(ErrorKind::InvalidIndex)?;
            let entry: IndexEntry = serde_json::from_str(&line).context(ErrorKind::InvalidIndex)?;

            res.insert(entry.sum.version.clone(), entry);
        }

        Ok(res)
    }
}
