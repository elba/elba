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

use super::backend::Backend;
use failure::{Error, ResultExt};
use indexmap::IndexMap;
use package::{
    
    version::Constraint,
    *,
};
use remote::resolution::{DirectRes, IndexRes, Resolution};
use semver::Version;
use serde_json;
use std::{
    fs,
    io::{self, prelude::*, BufReader},
    str::FromStr,
};
use toml;
use util::{
    errors::{ErrorKind, Res},
    lock::DirLock,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct IndexConfig {
    pub index: IndexConfInner,
}

impl FromStr for IndexConfig {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw)
            .context(format_err!("invalid index config"))
            .map_err(Error::from)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IndexConfInner {
    pub secure: bool,
    pub dependencies: IndexMap<String, IndexRes>,
    pub backend: Option<Backend>,
}

impl Default for IndexConfInner {
    fn default() -> Self {
        IndexConfInner {
            secure: false,
            dependencies: indexmap!(),
            backend: None,
        }
    }
}

/// A dependency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Dep<T> {
    pub name: Name,
    pub index: T,
    pub req: Constraint,
}

pub type ResolvedDep = Dep<IndexRes>;
pub type TomlDep = Dep<Option<String>>;

#[derive(Debug, Default)]
pub struct Indices {
    /// The indices being used.
    ///
    /// It is assumed that all dependent indices have been resolved, and that this mapping contains
    /// every index mentioned or depended on.
    pub indices: IndexMap<IndexRes, Index>,
    pub cache: IndexMap<PackageId, IndexMap<Version, ResolvedEntry>>,
}

impl Indices {
    pub fn new(indices: Vec<Index>) -> Self {
        let indices = indices.into_iter().map(|i| (i.id.clone(), i)).collect();
        let cache = indexmap!();

        Indices { indices, cache }
    }

    pub fn select_by_spec(&self, spec: &Spec) -> Res<Summary> {
        // For simplicity's sake, we don't do any caching here. It's not really necessary.
        let mut res = None;
        for (ir, ix) in &self.indices {
            if spec.resolution.is_none() || Some(&ir.clone().into()) == spec.resolution.as_ref() {
                if let Ok(es) = ix.entries(&spec.name) {
                    // We don't want to give back yanked packages
                    if let Some(x) = es
                        .into_iter()
                        .filter(|x| {
                            !x.1.yanked
                                && (spec.version.is_none()
                                    || Some(&x.1.version) == spec.version.as_ref())
                        }).last()
                    {
                        if let Some(existing) = res {
                            return Err(format_err!(
                                "spec `{}` is ambiguous, and matches both {} and {}@{}|{}",
                                &spec,
                                existing,
                                &spec.name,
                                ir,
                                x.0
                            ));
                        } else {
                            res = Some(Summary::new(
                                PackageId::new(spec.name.clone(), ir.clone().into()),
                                x.0,
                            ));
                        }
                    }
                }
            }
        }

        Ok(res.ok_or_else(|| ErrorKind::PackageNotFound)?)
    }

    pub fn select(&mut self, pkg: &Summary) -> Res<&ResolvedEntry> {
        let entry = self
            .entries(pkg.id())?
            .get(pkg.version())
            .ok_or_else(|| ErrorKind::PackageNotFound)?;

        Ok(entry)
    }

    // This assumes that the packages have already been loaded into the cache.
    pub fn count_versions(&self, pkg: &PackageId) -> usize {
        self.cache.get(pkg).map(|m| m.len()).unwrap_or(0)
    }

    pub fn entries(&mut self, pkg: &PackageId) -> Res<&IndexMap<Version, ResolvedEntry>> {
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
pub struct IndexEntry<T> {
    pub name: Name,
    pub version: Version,
    pub dependencies: Vec<Dep<T>>,
    pub yanked: bool,
    pub location: DirectRes,
}

pub type ResolvedEntry = IndexEntry<IndexRes>;
pub type TomlEntry = IndexEntry<Option<String>>;

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
    pub fn from_disk(res: DirectRes, path: DirLock) -> Res<Self> {
        let id = IndexRes { res };
        let pn = path.path().join("index.toml");
        let file = fs::File::open(&pn)
            .with_context(|e| format_err!("couldn't open index config {}: {}", pn.display(), e))?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|e| format_err!("couldn't read index config {}: {}", pn.display(), e))?;
        let config = IndexConfig::from_str(&contents)?;

        Ok(Index { id, path, config })
    }

    pub fn entries(&self, name: &Name) -> Res<IndexMap<Version, ResolvedEntry>> {
        let mut res = indexmap!();
        let path = self.path.path().join(name.as_normalized());
        let file = fs::File::open(path).context(ErrorKind::PackageNotFound)?;
        let r = io::BufReader::new(&file);

        for (lix, line) in r.lines().enumerate() {
            let entry: TomlEntry = serde_json::from_str(&line?).context(format_err!(
                "index entry {} for package {} is invalid",
                lix + 1,
                name
            ))?;

            let dependencies = entry
                .dependencies
                .into_iter()
                .map(|x| {
                    let index = x
                        .index
                        .and_then(|ix| self.config.index.dependencies.get(&ix))
                        .cloned()
                        .unwrap_or_else(|| self.id.clone());
                    Dep {
                        index,
                        name: x.name,
                        req: x.req,
                    }
                }).collect::<Vec<_>>();

            let entry: ResolvedEntry = IndexEntry {
                name: entry.name,
                version: entry.version,
                dependencies,
                yanked: entry.yanked,
                location: entry.location,
            };

            res.insert(entry.version.clone(), entry);
        }

        Ok(res)
    }

    pub fn depends(&self) -> impl Iterator<Item = &IndexRes> {
        self.config.index.dependencies.iter().map(|x| x.1)
    }

    pub fn backend(&self) -> Option<&Backend> {
        self.config.index.backend.as_ref()
    }
}
