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
use package::{manifest::Manifest, resolution::DirectRes, PackageId};
use reqwest::Client;
use sha2::{Digest, Sha256};
use slog::Logger;
use std::{
    collections::VecDeque,
    fs,
    io::{prelude::*, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use util::{
    clear_dir, copy_dir,
    errors::{ErrorKind, Res},
    graph::Graph,
    hexify_hash,
    lock::DirLock,
};
use walkdir::WalkDir;

/// The Cache encapsulates all of the global state required for `elba` to function.
///
/// This global state includes stuff like temporary places to download and build packages, places
/// to store indices of packages, etc.
///
/// Note that a Cache can be located anywhere, including in the current directory!
#[derive(Debug, Clone)]
pub struct Cache {
    pub layout: Layout,
    client: Client,
    pub logger: Logger,
}

impl Cache {
    pub fn from_disk(plog: &Logger, location: &Path) -> Res<Self> {
        let layout = Layout::new(&location)?;

        let client = Client::new();
        let logger = plog.new(o!("location" => location.to_string_lossy().into_owned()));

        Ok(Cache {
            layout,
            client,
            logger,
        })
    }

    /// Retrieve the metadata of a package, loading it into the cache if necessary.
    pub fn checkout_source(&self, pkg: &PackageId, loc: &DirectRes) -> Result<Source, Error> {
        let p = self.load_source(loc)?;

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
    fn load_source(&self, loc: &DirectRes) -> Result<DirLock, Error> {
        if let Some(path) = self.check_source(loc) {
            DirLock::acquire(&path)
        } else {
            let p = self.layout.src.join(Self::get_source_dir(loc));

            let dir = DirLock::acquire(&p)?;
            loc.retrieve(&self.client, &dir)?;

            Ok(dir)
        }
    }

    /// Check if package is downloaded and in the cache. If so, returns the path of source of the cached
    /// package.
    fn check_source(&self, loc: &DirectRes) -> Option<PathBuf> {
        if let DirectRes::Dir { url } = loc {
            return Some(url.clone());
        }

        let path = self.layout.src.join(Self::get_source_dir(loc));
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
    fn get_source_dir(loc: &DirectRes) -> String {
        let mut hasher = Sha256::default();
        hasher.input(loc.to_string().as_bytes());
        hexify_hash(hasher.result().as_slice())
    }

    /// If the build directory exists, returns it. Otherwise, give up and return None
    pub fn checkout_build(&self, hash: &BuildHash) -> Res<Option<Binary>> {
        if let Some(path) = self.check_build(&hash) {
            Ok(Some(Binary {
                target: DirLock::acquire(&path)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Returns a lock on a temporary build directory.
    /// Note that the format of this directory should be an OutputLayout.
    pub fn checkout_tmp(&self, hash: &BuildHash) -> Res<OutputLayout> {
        let path = self.layout.tmp.join(&hash.0);
        let lock = DirLock::acquire(&path)?;
        if lock.path().exists() {
            clear_dir(&lock.path()).context(format_err!("couldn't remove existing output path"))?;
        }
        OutputLayout::new(lock)
    }

    pub fn store_build(&self, from: &Path, hash: &BuildHash) -> Res<Binary> {
        let dest = self.layout.build.join(&hash.0);

        if !dest.exists() {
            fs::create_dir_all(&dest)?;
        }

        let dest = DirLock::acquire(&dest)?;

        clear_dir(dest.path())?;
        copy_dir(from, dest.path())?;

        Ok(Binary { target: dest })
    }

    fn check_build(&self, hash: &BuildHash) -> Option<PathBuf> {
        let path = self.layout.root.to_owned();
        let path = path.join("build").join(&hash.0);

        if path.exists() {
            Some(path)
        } else {
            None
        }
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

/// Layouts encapsulate the logic behind our directory structure.
#[derive(Debug, Clone)]
pub struct Layout {
    /// Root directory of the Layout
    pub root: PathBuf,
    /// Build location of codegen'd libraries
    pub artifacts: PathBuf,
    /// Location to dump binaries
    pub bin: PathBuf,
    /// Source download directory
    pub src: PathBuf,
    /// Built library (ibc output) directory
    pub build: PathBuf,
    /// Temporary build directory
    pub tmp: PathBuf,
    /// Directory of all the indices
    pub indices: PathBuf,
}

impl Layout {
    pub fn new(root: &Path) -> Res<Self> {
        let layout = Layout {
            root: root.to_path_buf(),
            bin: root.join("bin"),
            artifacts: root.join("artifacts"),
            src: root.join("src"),
            build: root.join("build"),
            indices: root.join("indices"),
            tmp: root.join("tmp"),
        };

        // create_dir_all ignores pre-existing folders
        fs::create_dir_all(&layout.root)?;
        fs::create_dir_all(&layout.artifacts)?;
        fs::create_dir_all(&layout.bin)?;
        fs::create_dir_all(&layout.src)?;
        fs::create_dir_all(&layout.build)?;
        fs::create_dir_all(&layout.indices)?;
        fs::create_dir_all(&layout.tmp)?;

        Ok(layout)
    }
}

/// The Layout of an output directory. This is used either as the `target` directory or one of the
/// folders in a temporary build directory in the global cache.
#[derive(Debug)]
pub struct OutputLayout {
    lock: DirLock,
    pub root: PathBuf,
    pub artifacts: PathBuf,
    pub bin: PathBuf,
    pub lib: PathBuf,
    pub build: PathBuf,
    pub deps: PathBuf,
    pub hash: Option<BuildHash>,
}

impl OutputLayout {
    pub fn new(lock: DirLock) -> Res<Self> {
        let root = lock.path().to_path_buf();

        let layout = OutputLayout {
            lock,
            root: root.clone(),
            artifacts: root.join("artifacts"),
            bin: root.join("bin"),
            lib: root.join("lib"),
            build: root.join("build"),
            deps: root.join("deps"),
            hash: fs::read(root.join("hash"))
                .map(|x| BuildHash(String::from_utf8_lossy(&x).to_string()))
                .ok(),
        };

        // create_dir_all ignores pre-existing folders
        fs::create_dir_all(&layout.root)?;
        fs::create_dir_all(&layout.bin)?;
        fs::create_dir_all(&layout.lib)?;
        fs::create_dir_all(&layout.build)?;
        fs::create_dir_all(&layout.deps)?;

        Ok(layout)
    }

    pub fn write_hash(&self, hash: &BuildHash) -> Res<()> {
        fs::write(self.root.join("hash"), hash.0.as_bytes())
            .context(format_err!("couldn't write hash"))?;

        Ok(())
    }

    pub fn is_built(&self, hash: &BuildHash) -> bool {
        self.hash.as_ref() == Some(hash)
    }
}

/// Information about the source of package that is available somewhere in the file system.
/// Packages are stored as directories on disk (not archives because it would just be a bunch of
/// pointless unpacking-repacking).
#[derive(Debug, Clone)]
pub struct Source {
    // Note: the reason we have to deal with this Arc is because in the JobQueue, there's no way of
    // moving Sources out of the queue, hence the need to clone the references to the Source.
    // TODO: Get rid of this Arc somehow
    inner: Arc<SourceInner>,
}

#[derive(Debug)]
struct SourceInner {
    /// The package's manifest
    meta: Manifest,
    location: DirectRes,
    /// The path to the package.
    path: DirLock,
    hash: String,
}

impl Source {
    /// Creates a Source from a folder on disk.
    ///
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
    // TODO: Ignore `target/` folder!!!
    pub fn from_folder(pkg: &PackageId, path: DirLock, location: DirectRes) -> Res<Self> {
        let mf_path = path.path().join("elba.toml");

        let file = fs::File::open(mf_path).context(ErrorKind::MissingManifest)?;
        let mut file = BufReader::new(file);
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context(ErrorKind::InvalidIndex)?;

        let manifest = Manifest::from_str(&contents).context(ErrorKind::InvalidIndex)?;

        if let Some(p) = manifest.workspace.get(pkg.name()) {
            let lock = DirLock::acquire(&path.path().join(&p.0))?;
            // We immediately release our lock on the parent folder
            drop(path);
            return Source::from_folder(pkg, lock, location);
        }

        if manifest.summary().name() != pkg.name() {
            bail!(
                "names don't match: {} was declared, but {} was found in elba.toml",
                pkg.name(),
                manifest.summary().name()
            )
        }

        // Creating the hash
        let walker = WalkDir::new(path.path()).into_iter().filter_entry(|entry| {
            entry.file_name() != "target" &&
            entry.file_name()
                .to_str()
                .map(|s| !s.starts_with("."))
                .unwrap_or(false) && entry.file_type().is_file() 
        });
        
        let mut hash = Sha256::new();
        for f in walker {
            let mut file = fs::File::open(f.unwrap().path())?;
            let fh = Sha256::digest_reader(&mut file)?;
            hash.input(&fh);
        }
        let hash = hexify_hash(hash.result().as_slice());

        Ok(Source {
            inner: Arc::new(SourceInner {
                meta: manifest,
                location,
                path,
                hash,
            }),
        })
    }

    pub fn meta(&self) -> &Manifest {
        &self.inner.meta
    }

    pub fn location(&self) -> &DirectRes {
        &self.inner.location
    }

    pub fn hash(&self) -> &str {
        &self.inner.hash
    }

    pub fn path(&self) -> &Path {
        self.inner.path.path()
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

impl Eq for Source {}

/// Information about a built library that is available somewhere in the file system.
#[derive(Debug, PartialEq, Eq)]
pub struct Binary {
    pub target: DirLock,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BuildHash(String);

impl BuildHash {
    pub fn new(root: &Source, sources: &Graph<Source>) -> Self {
        let mut hasher = Sha256::default();
        for (_, src) in sources.sub_tree(sources.find_id(root).unwrap()) {
            hasher.input(&src.hash().as_bytes());
        }
        let hash = hexify_hash(hasher.result().as_slice());
        BuildHash(hash)
    }
}
