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

use util::err::{Error, ErrorKind};
use failure::ResultExt;
use indexmap::IndexMap;
use package::{
    manifest::{DepReq, Manifest},
    resolution::{DirectRes, IndexRes},
    version::Constraint,
    Name, PackageId, Summary
};
use reqwest::Client;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{prelude::*, BufReader},
    path::PathBuf,
    str::FromStr,
};
use util::{hexify_hash, lock::DirLock};

/// Metadata for a package in the Cache.
///
/// Note that if a root depends directly on a git repo or path, it doesn't necessarily have a
/// Constraint (the constraint is contained in the Resolution - use *this* directory or *this*
/// git commit), so for those packages the Constraint is just "any."
pub struct CacheMeta {
    pub version: Version,
    pub deps: IndexMap<PackageId, Constraint>,
}

/// A Cache of downloaded packages and packages with no other Index.
///
/// A Cache is located in a directory, and it has two directories of its own:
/// - `src/`: the cache of downloaded packages, in full source form.
/// - `build/`: the cache of built packages.
///
/// The src and build folders contain one folder for every package on disk.
// TODO: Maybe follow Cargo and separate into two cache locations: global (`.cargo/registry`:
// contains caches and metadata for indices) and local (`target/`: contains non-index deps, built
// versions of all packages i.e. ibc files, etc.).
// TODO: Dealing with people using multiple executions of `elba` at once: make sure that one Cache
// doesn't clobber another. See Cargo's `util/flock.rs`, the fs2 crate, etc. This way, multiple
// copies of `elba` don't try copying over each other, etc.
// Simple solution: use a lock file. If test.lock exists, it's locked. Otherwise, we create it
// and now we have the lock.
// TODO: Maybe the Cache is in charge of the Indices. This way, metadata takes into account both
// indices and direct deps, and we don't have to discriminate between the two in the Retriever.
#[derive(Debug, Clone)]
pub struct Cache {
    location: PathBuf,
    def_index: IndexRes,
    client: Client,
}

impl Cache {
    pub fn from_disk(location: PathBuf, def_index: IndexRes) -> Self {
        let mut loc = location.clone();
        loc.push("dl");
        let _ = fs::create_dir_all(&loc);

        loc.pop();
        loc.push("src");
        let _ = fs::create_dir_all(&loc);

        loc.pop();
        loc.push("build");
        let _ = fs::create_dir_all(&loc);

        let client = Client::new();

        Cache {
            location,
            def_index,
            client,
        }
    }

    /// Retrieve the metadata of a package, loading it into the cache if necessary. This should
    /// only be used for non-index dependencies.
    pub fn metadata(&self, pkg: &PackageId, loc: &DirectRes, v: Option<&Version>) -> Result<CacheMeta, Error> {
        let p = self.load(pkg, loc, v)?;
        let mf_path = p.join("Cargo.toml");

        let file = fs::File::open(mf_path).context(ErrorKind::MissingManifest)?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context(ErrorKind::InvalidIndex)?;

        let manifest = Manifest::from_str(&contents).context(ErrorKind::InvalidIndex)?;
        let version = manifest.version().clone();
        let mut deps = indexmap!();

        // We ignore dev-dependencies because those are only relevant if that package is the root
        for (n, dep) in manifest.dependencies {
            let (pid, c) = self.depreq_to_tuple(n, dep);
            deps.insert(pid, c);
        }

        let meta = CacheMeta { deps, version };

        Ok(meta)
    }

    // TODO: In the future (heh), return Box<Future<Item = PathBuf, Error = Error>> and use async
    // reqwest. For now, it seems like too much trouble for not that much gain.
    // Info on async:
    // https://stackoverflow.com/questions/49087958/getting-multiple-urls-concurrently-with-hyper
    // Info on downloading things in general:
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/download.html
    // TODO: Return dirlock?
    /// Returns a future pointing to the path to a downloaded (and potentially extracted, if it's a
    /// tarball) package.
    ///
    /// If the package has been cached, this function does no I/O. If it hasn't, it goes wherever
    /// it needs to in order to retrieve the package.
    ///
    /// If the package has a direct resolution of a local file directory, this just symlinks the
    /// package into the directory of the cache.
    pub fn load(&self, pkg: &PackageId, loc: &DirectRes, v: Option<&Version>) -> Result<PathBuf, Error> {
        if let Some(path) = self.check(pkg.name(), loc, v) {
            Ok(path)
        } else {
            let mut p = self.location.clone();
            p.push("src");
            p.push(Self::get_src_dir(pkg.name(), loc, v));

            // TODO: Oh my god this is unsafe pls pls pls fix
            let dir = DirLock::acquire(&p).unwrap();
            loc.retrieve(&self.client, &dir)?;
            dir.release().unwrap();

            Ok(p)
        }
    }

    /// Check if package is downloaded and in the cache. If so, returns the path of the cached
    /// package.
    pub fn check(&self, name: &Name, loc: &DirectRes, v: Option<&Version>) -> Option<PathBuf> {
        let mut path = self.location.clone();
        path.push("src");
        path.push(Self::get_src_dir(name, loc, v));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Gets the corresponding directory of a package. We need this because for packages which have
    /// no associated version (i.e. git and local dependencies, where the constraints are inherent
    /// in the resolution itself), we ignore a version specifier.
    fn get_src_dir(name: &Name, loc: &DirectRes, v: Option<&Version>) -> String {
        let mut hasher = Sha256::default();
        hasher.input(name.as_bytes());
        hasher.input(loc.to_string().as_bytes());
        if let Some(v) = v {
            hasher.input(v.to_string().as_bytes());
        }
        let hash = hexify_hash(hasher.result().as_slice());

        format!("{}_{}-{}", name.group(), name.name(), hash)
    }

    /// Gets the corresponding directory of a built package (with ibc files). This directory is
    /// different from the directory for downloads because the hash takes into consideration more
    /// factors, like the complete environment that the package was built in (i.e. all of the
    /// exact dependencies used for this build of the package).
    /// 
    /// This is necessary because Idris libraries can re-export values of its dependencies; when a
    /// dependent value changes, it changes in the library itself, causing the generated ibc to be
    /// totally different. The same package with the same constraints can be resolved with
    /// different versions in different contexts, so we want to make sure we're using the right
    /// builds of every package.
    fn get_build_dir(sum: &Summary, loc: &DirectRes, env: Vec<(PackageId, Version)>) -> String {
        unimplemented!()
    }

    fn depreq_to_tuple(&self, n: Name, i: DepReq) -> (PackageId, Constraint) {
        match i {
            DepReq::Registry(c) => {
                let pi = PackageId::new(n, self.def_index.clone().into());
                (pi, c)
            }
            DepReq::RegLong { con, registry } => {
                let pi = PackageId::new(n, registry.into());
                (pi, con)
            }
            DepReq::Local { path } => {
                let res = DirectRes::Dir { url: path };
                let pi = PackageId::new(n, res.into());
                (pi, Constraint::any())
            }
            DepReq::Git { git, spec } => unimplemented!(),
        }
    }
}
