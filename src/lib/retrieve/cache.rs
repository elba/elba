//! Caching packages which have been downloaded before.
//!
//! ## Background: Previous design
//! Previous designs for `elba` indices alluded to a feature where the local package cache would
//! be formatted as an index itself, with its entries pointing to locations on disk where
//! downloaded packages (whether as a result of DirectRes deps by root or through Indices)
//! are located. However, this relied on the concept of "index overlapping" in which, in the case
//! of multiple indices having the same name, the package from the "higher priority" index would be
//! picked. In this previous design, the package from the "cache index" would be picked, avoiding
//! re-downloading of packages.
//!
//! However, this design of overlapping indices was abandoned because it made package resolution
//! unreliable and dependent on global state. Additionally, because an Index can only store and
//! manage metadata information, a separate Cache struct would've been needed anyway to manage
//! caching, making this design complex and complicated.
//!
//! ## Current design
//! In this new design, there is a much clearer separation between storing package metadata from
//! remote sources (handled strictly by Indices) and caching the packages themselves locally
//! (which is handled by the Cache struct). The Cache struct is responsible for determining if a
//! package has already been retrieved from the Internet, and coordinates cached package downloads.
//!
//! The Cache struct must be responsible for a directory which contains previously
//! downloaded packages from all sources, and should deal with checksums and things like that to see if
//! a redownload is needed. Whenever a package is about to be downloaded, the Cache is there to see
//! if it really actually needs to be downloaded.
//!
//! The Cache doesn't need its own Index; the point of an Index is to cache metadata about packages,
//! but the Cache already has fully downloaded packages with manifests included, so it can just
//! peek at the manifests to find out about package information. Every package will get a directory
//! according to its summary, which is how the Cache will know what packages are available. Git repos
//! should be cloned into the path of the Cache, and local dir dependencies should be symlinked in.
//!
//! ### Future potential
//! This new design for the cache makes possible several desirable features which could easily be
//! implemented in the future.
//!
//! #### "Airplane mode"
//! If a user does not want to access the Internet to resolve packages, `elba` can limit itself
//! to only using the packages provided by the Cache.
//!
//! #### Vendoring
//! In order to vendor packages, `elba` can create a new Cache in the project directory and require
//! that all packages originate from the vendor directory (basically airplane mode + custom cache
//! directory). Directory dependencies should be copied into the Cache directory unconditionally.
//! From there, the user should change their manifest so that it points to the vendored directory.
//!
//! #### Build caching
//! If we want to cache builds, we can just have a separate subfolder for ibcs.

use failure::{Error, ResultExt};
use index::{Index, Indices};
use package::{manifest::Manifest, resolution::DirectRes, Name, PackageId};
use reqwest::Client;
use semver::Version;
use sha2::{Digest, Sha256};
use slog::Logger;
use std::{
    collections::VecDeque,
    env, fs,
    io::{prelude::*, BufReader},
    path::PathBuf,
    str::FromStr,
};
use tar::Builder;
use util::{
    copy_dir,
    errors::{ErrorKind, Res},
    graph::Graph,
    hexify_hash,
    lock::DirLock,
};

/// A Cache of downloaded packages and packages with no other Index.
///
/// A Cache is located in a directory, and it has two directories of its own:
/// - `src/`: the cache of downloaded packages, in full source form.
/// - `build/`: the cache of built packages.
///
/// The src and build folders contain one folder for every package on disk.
// TODO: Maybe the Cache is in charge of the Indices. This way, metadata takes into account both
// indices and direct deps, and we don't have to discriminate between the two in the Retriever.
#[derive(Debug, Clone)]
pub struct Cache {
    layout: Layout,
    client: Client,
    pub logger: Logger,
}

impl Cache {
    pub fn from_disk(plog: &Logger, location: PathBuf) -> Self {
        let layout = Layout::new(&location).unwrap();
        let client = Client::new();
        let logger = plog.new(o!("location" => location.to_string_lossy().into_owned()));

        Cache {
            layout,
            client,
            logger,
        }
    }

    /// Retrieve the metadata of a package, loading it into the cache if necessary.
    pub fn checkout_source(
        &self,
        pkg: &PackageId,
        loc: &DirectRes,
        v: Option<&Version>,
    ) -> Result<Source, Error> {
        let p = self.load_source(pkg, loc, v)?;

        Source::from_folder(pkg, p, loc.clone())
    }

    // TODO: In the future (heh), return Box<Future<Item = PathBuf, Error = Error>> and use async
    // reqwest. For now, it seems like too much trouble for not that much gain.
    // Info on async:
    // https://stackoverflow.com/questions/49087958/getting-multiple-urls-concurrently-with-hyper
    // Info on downloading things in general:
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html
    /// Returns a future pointing to the path to a downloaded (and potentially extracted, if it's a
    /// tarball) package.
    ///
    /// If the package has been cached, this function does no I/O. If it hasn't, it goes wherever
    /// it needs to in order to retrieve the package.
    fn load_source(
        &self,
        pkg: &PackageId,
        loc: &DirectRes,
        v: Option<&Version>,
    ) -> Result<DirLock, Error> {
        if let Some(path) = self.check_source(pkg.name(), loc, v) {
            DirLock::acquire(&path)
        } else {
            let path = self
                .layout
                .src
                .join(Self::get_source_dir(pkg.name(), loc, v));
            let dir = DirLock::acquire(&path)?;
            loc.retrieve(&self.client, &dir)?;

            Ok(dir)
        }
    }

    // TODO: Workspaces for git repos.
    /// Check if package is downloaded and in the cache. If so, returns the path of source of the cached
    /// package.
    fn check_source(&self, name: &Name, loc: &DirectRes, v: Option<&Version>) -> Option<PathBuf> {
        if let DirectRes::Dir { url } = loc {
            return Some(url.clone());
        }

        let path = self.layout.src.join(Self::get_source_dir(name, loc, v));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Gets the corresponding directory of a package. We need this because for packages which have
    /// no associated version (i.e. git and local dependencies, where the constraints are inherent
    /// in the resolution itself), we ignore a version specifier.
    ///
    /// Note: with regard to git repos, we treat the same repo with different checked out commits/
    /// tags as completely different repos.
    fn get_source_dir(name: &Name, loc: &DirectRes, v: Option<&Version>) -> String {
        let mut hasher = Sha256::default();
        hasher.input(name.as_bytes());
        hasher.input(loc.to_string().as_bytes());
        if let Some(v) = v {
            // We only care about the version of the package at this source directory if it came
            // from a tarball
            if let DirectRes::Tar {
                url: _url,
                cksum: _cksum,
            } = loc
            {
                hasher.input(v.to_string().as_bytes());
            }
        }
        let hash = hexify_hash(hasher.result().as_slice());

        format!("{}_{}-{}", name.group(), name.name(), hash)
    }

    // TODO: local `target/` dir.
    /// Acquires a lock on a build directory - either the directory of the actual or a tmp dir
    pub fn checkout_build(
        &self,
        root: &Source,
        sources: &Graph<Source>,
        local: bool,
    ) -> Result<Binary, Error> {
        if let Some(path) = self.check_build(&root, sources, local) {
            Ok(Binary::built(DirLock::acquire(&path)?))
        } else {
            let tp = self.layout.tmp.join(Self::get_build_dir(&root, sources));

            let bp = self.layout.build.join(Self::get_build_dir(&root, sources));

            let tl = DirLock::acquire(&tp)?;
            copy_dir(root.path.path(), tl.path())?;
            let bl = DirLock::acquire(&bp)?;

            Ok(Binary::new(root.with_path(tl), bl))
        }
    }

    fn check_build(&self, root: &Source, sources: &Graph<Source>, local: bool) -> Option<PathBuf> {
        let path = if local {
            env::current_dir().ok()?.join("target")
        } else {
            self.layout.root.clone()
        };
        let path = path.join("build").join(Self::get_build_dir(root, sources));

        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    fn get_build_dir(root: &Source, sources: &Graph<Source>) -> String {
        let mut hasher = Sha256::default();

        for (_, src) in sources.sub_tree(root).unwrap() {
            hasher.input(&src.hash.as_bytes());
        }

        hexify_hash(hasher.result().as_slice())
    }

    // TODO: We do a lot of silent erroring. Is that good?
    pub fn get_indices(&self, index_reses: &[DirectRes]) -> Indices {
        let mut indices = vec![];
        let mut seen = vec![];
        let mut q: VecDeque<DirectRes> = index_reses.iter().cloned().collect();

        while let Some(index) = q.pop_front() {
            if seen.contains(&index) {
                continue;
            }
            // We special-case a local dir index because `dir` won't exist for it.
            if let DirectRes::Dir { url } = &index {
                let lock = if let Ok(dir) = DirLock::acquire(url) {
                    dir
                } else {
                    continue;
                };

                let ix = Index::from_disk(index.clone(), lock);
                if let Ok(ix) = ix {
                    for dependent in ix.depends().iter().cloned().map(|i| i.res) {
                        q.push_back(dependent);
                    }
                    seen.push(index);
                    indices.push(ix);
                }
                continue;
            }

            let dir = if let Ok(dir) =
                DirLock::acquire(&self.layout.indices.join(Self::get_index_dir(&index)))
            {
                dir
            } else {
                continue;
            };

            if dir.path().exists() {
                let ix = Index::from_disk(index.clone(), dir);
                if let Ok(ix) = ix {
                    for dependent in ix.depends().iter().cloned().map(|i| i.res) {
                        q.push_back(dependent);
                    }
                    seen.push(index);
                    indices.push(ix);
                }
                continue;
            }

            if index.retrieve(&self.client, &dir).is_ok() {
                let ix = Index::from_disk(index.clone(), dir);
                if let Ok(ix) = ix {
                    for dependent in ix.depends().iter().cloned().map(|i| i.res) {
                        q.push_back(dependent);
                    }
                    seen.push(index);
                    indices.push(ix);
                }
            }
        }

        Indices::new(indices)
    }

    fn get_index_dir(loc: &DirectRes) -> String {
        let mut hasher = Sha256::default();
        hasher.input(loc.to_string().as_bytes());
        hexify_hash(hasher.result().as_slice())
    }
}

#[derive(Debug, Clone)]
struct Layout {
    pub root: PathBuf,
    pub src: PathBuf,
    pub build: PathBuf,
    pub indices: PathBuf,
    pub tmp: PathBuf,
}

impl Layout {
    pub fn new(root: &PathBuf) -> Result<Self, Error> {
        let layout = Layout {
            root: root.to_path_buf(),
            src: root.join("src"),
            build: root.join("build"),
            indices: root.join("indices"),
            tmp: root.join("tmp"),
        };

        fs::create_dir_all(&layout.root)?;
        fs::create_dir_all(&layout.src)?;
        fs::create_dir_all(&layout.build)?;
        fs::create_dir_all(&layout.indices)?;
        fs::create_dir_all(&layout.tmp)?;

        Ok(layout)
    }
}

/// Information about the source of package that is available somewhere in the file system.
/// Packages are stored as directories on disk (not archives because it would just be a bunch of
/// pointless unpacking-repacking).
#[derive(Debug)]
pub struct Source {
    /// The package's manifest
    pub meta: Manifest,
    pub location: DirectRes,
    /// The path to the package.
    pub path: DirLock,
    pub hash: String,
}

impl Source {
    /// The purpose of having a hash is for builds. The resolution graph only stores Summaries. If we were
    /// to rely solely on hashing the Summaries of a package's dependencies to determine if we need
    /// to rebuild a package, we'd run into a big problem: a package would only get rebuilt iff its
    /// own version changed or a version of one of its dependents changed. This is a problem for
    /// DirectRes deps, since they can change often without changing their version, leading to
    /// erroneous cases where packages aren't rebuilt. Even if we were to use the hash that
    /// determines the folder name of a package, it wouldn't be enough. Local dependencies' folder
    /// names never change and don't have a hash, and git repos which pin themselves to a branch
    /// can maintain the same hash while updating their contents.
    ///
    /// Not only that, but stray ibc files and other miscellaneous crap could get into the source
    /// directory, which would force us to rebuild yet again.
    ///
    /// To remedy this, we'd like to have a hash that indicates that the file contents of a Source
    /// have changed.
    ///
    /// Note that the hash stored differs from the hash used to determine if a package needs to be
    /// redownloaded completely; for git repos, if the resolution is to use master, then the same
    /// folder will be used, but will be checked out to the latest master every time.
    pub fn from_folder(pkg: &PackageId, path: DirLock, location: DirectRes) -> Res<Self> {
        let mf_path = path.path().join("elba.toml");

        let file = fs::File::open(mf_path).context(ErrorKind::MissingManifest)?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context(ErrorKind::InvalidIndex)?;

        let manifest = Manifest::from_str(&contents).context(ErrorKind::InvalidIndex)?;

        if manifest.summary().name() != pkg.name() {
            bail!(
                "names don't match: {} was declared, but {} was found in elba.toml",
                pkg.name(),
                manifest.summary().name()
            )
        }

        // Pack into a tar file to hash it quickly
        let f = fs::File::create(path.path().with_extension("tar"))?;
        let mut ar = Builder::new(f);
        ar.append_dir_all("irrelevant", path.path())?;

        let _ = ar.into_inner()?;

        let p = path.path().with_extension("tar");
        let mut file = fs::File::open(&p)?;
        let result = Sha256::digest_reader(&mut file)?;
        let hash = hexify_hash(result.as_slice());

        // The tarball is useless to us now.
        drop(file);
        fs::remove_file(p)?;

        Ok(Source {
            meta: manifest,
            location,
            path,
            hash,
        })
    }

    pub fn with_path(&self, p: DirLock) -> Source {
        Source {
            meta: self.meta.clone(),
            location: self.location.clone(),
            hash: self.hash.clone(),
            path: p,
        }
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for Source {}

/// Information about the build of library that is available somewhere in the file system.
#[derive(Debug, PartialEq, Eq)]
pub struct Binary {
    source: Option<Source>,
    target: DirLock,
}

impl Binary {
    pub fn is_complete(&self) -> bool {
        self.source.is_some()
    }

    pub fn built(lock: DirLock) -> Self {
        Binary {
            source: None,
            target: lock,
        }
    }

    pub fn new(source: Source, target: DirLock) -> Self {
        Binary {
            source: Some(source),
            target,
        }
    }
}
