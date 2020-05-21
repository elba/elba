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

use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{self, prelude::*, BufReader},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use console::style;
use failure::{bail, format_err, ResultExt};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use slog::{debug, o, Logger};
use toml;
use walkdir::WalkDir;

use crate::{
    build::{context::BuildContext, Targets},
    cli::build::find_manifest,
    package::{manifest::Manifest, PackageId, Spec},
    remote::{
        resolution::{DirectRes, Resolution},
        Index, Indices,
    },
    util::{
        clear_dir, copy_dir,
        error::Result,
        graph::Graph,
        lock::DirLock,
        shell::{Shell, Verbosity},
        valid_file,
    },
};

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
    pub shell: Shell,
}

impl Cache {
    pub fn from_disk(plog: &Logger, layout: Layout, shell: Shell) -> Result<Self> {
        layout.init()?;

        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let logger = plog.new(o!("phase" => "cache"));

        Ok(Cache {
            layout,
            client,
            logger,
            shell,
        })
    }

    /// Retrieve the metadata of a package, loading it into the cache if necessary.
    pub fn checkout_source(
        &self,
        pkg: &PackageId,
        loc: &DirectRes,
        eager: bool,
        offline: bool,
        dl_f: impl Fn(),
    ) -> Result<(Option<DirectRes>, Source)> {
        let p = self.load_source(pkg, loc, eager, offline, dl_f)?;

        Ok((p.0, Source::from_folder(pkg, p.1, loc.clone())?))
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
    ///
    /// We take both the PackageId and the DirectRes because of git repositories. The PackageId will
    /// be the package as declared in the manifest, while the DirectRes will be the git resolution
    /// in the lockfile. If a lockfile doesn't exist, or the manifest and lockfile have the exact
    /// same git resolution, the pkg argument won't be used.
    fn load_source(
        &self,
        pkg: &PackageId,
        loc: &DirectRes,
        eager: bool,
        offline: bool,
        dl_f: impl Fn(),
    ) -> Result<(Option<DirectRes>, DirLock)> {
        if let DirectRes::Dir { path } = loc {
            debug!(self.logger, "loaded source"; "cause" => "dir", "pkg" => pkg.to_string());
            return Ok((None, DirLock::acquire(&path)?));
        }

        let eager = if offline { false } else { eager };

        let new_dir = self.layout.src.join(Self::get_source_dir(loc, true));
        // We record first if the directory existed before the retrieval process

        // If it does exist, we can stop immediately
        if loc.is_tar() && new_dir.exists() {
            debug!(
                self.logger, "loaded source";
                "cause" => "exists",
                "pkg" => pkg.to_string(),
                "dir" => new_dir.display()
            );
            return Ok((None, DirLock::acquire(&new_dir)?));
        }

        let new_f = |dl_online| {
            if offline && dl_online {
                return Err(format_err!("Can't download package in offline mode"));
            }
            dl_f();
            Ok(())
        };

        // At this point, we're only dealing with all git resolutions and tarball resolutions
        // which don't exist yet.
        // If we're in "offline" mode, we immediately return an error from here because we
        // won't be able to download anything anyways.
        let dir = DirLock::acquire(&self.layout.src.join(Self::get_source_dir(loc, true)))?;
        let res = if let Resolution::Direct(g) = pkg.resolution() {
            // For a git repository, if the DirectRes and the PackageId don't match, we should try to
            // retrieve the locked variant (the DirectRes) and then update with the latest variant
            // (the PackageId).
            // The only difference between the two should be the `tag`.
            // The worst case performance-wise for this operation is if a repository doesn't exist
            // in the cache and the lockfile and manifest are out-of-sync, which will result in
            // two fetch operations.
            if g.is_git() && g != loc {
                debug_assert!(loc.is_git());
                loc.retrieve(&self.client, &dir, eager, new_f)
                    .and_then(|_| {
                        g.retrieve(&self.client, &dir, false, |dl_online| {
                            if offline && dl_online {
                                Err(format_err!("Can't download package in offline mode"))
                            } else {
                                Ok(())
                            }
                        })
                    })
            } else {
                loc.retrieve(&self.client, &dir, eager, new_f)
            }
        } else {
            loc.retrieve(&self.client, &dir, eager, new_f)
        }?;

        let new_dir = self.layout.src.join(&Self::get_source_dir(
            if let Some(r) = res.as_ref() { r } else { &loc },
            true,
        ));
        let dir = if new_dir != dir.path() {
            if !new_dir.exists() {
                copy_dir(dir.path(), &new_dir, true)?;
            }
            DirLock::acquire(&new_dir)?
        } else {
            dir
        };

        debug!(
            self.logger, "loaded source";
            "cause" => "retrieved_new",
            "pkg" => pkg.to_string(),
            "loc" => loc.to_string(),
            "dir" => dir.path().display()
        );

        Ok((res, dir))
    }

    /// Gets the corresponding directory of a package.
    pub fn get_source_dir(loc: &DirectRes, include_tag: bool) -> String {
        let mut hasher = Sha256::default();
        if !include_tag {
            if let DirectRes::Git { repo, .. } = loc {
                hasher.input(repo.to_string().as_bytes());
            } else {
                hasher.input(loc.to_string().as_bytes());
            }
        } else {
            hasher.input(loc.to_string().as_bytes());
        }
        hex::encode(hasher.result())
    }

    /// Return the build directory exists, else None.
    pub fn checkout_build(&self, hash: &BuildHash) -> Result<Option<Binary>> {
        if let Some(path) = self.check_build(&hash) {
            Ok(Some(Binary::new(DirLock::acquire(&path)?)))
        } else {
            Ok(None)
        }
    }

    /// Returns a lock on a temporary build directory.
    /// Note that the format of this directory should be an OutputLayout.
    pub fn checkout_tmp(&self, hash: &BuildHash) -> Result<OutputLayout> {
        let path = self.layout.tmp.join(&hash.0);
        let lock = DirLock::acquire(&path)?;
        if lock.path().exists() {
            clear_dir(&lock.path()).context(format_err!(
                "couldn't remove existing output path: {}",
                lock.path().display()
            ))?;
        }
        OutputLayout::new(lock)
    }

    pub fn store_bins(&self, bins: &[(PathBuf, String)], force: bool) -> Result<()> {
        // We use a file .bins in the bin directory to keep track of installed bins
        let mut dot_f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(self.layout.bin.join(".bins"))
            .with_context(|e| format_err!("could not open .bins file:\n{}", e))?;

        let mut dot_c = String::new();
        dot_f
            .read_to_string(&mut dot_c)
            .with_context(|e| format_err!("could not read .bins file:\n{}", e))?;

        let mut dot: IndexMap<String, String> = toml::from_str(&dot_c)
            .with_context(|e| format_err!("could not deserialize .bins file:\n{}", e))?;

        for (path, sum) in bins {
            self.shell.println(
                style("Installing").cyan(),
                path.file_name().unwrap().to_string_lossy().as_ref(),
                Verbosity::Normal,
            );
            self.store_bin(path, force)?;
            dot.insert(
                // We can unwrap the file name; we checked for an error in store_bin
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_owned()
                    .to_string(),
                sum.to_string(),
            );
        }

        drop(dot_f);
        fs::remove_file(self.layout.bin.join(".bins"))
            .with_context(|e| format_err!("could not clear existing .bins file:\n{}", e))?;

        fs::write(
            self.layout.bin.join(".bins"),
            toml::to_string(&dot).unwrap().as_bytes(),
        )
        .with_context(|e| format_err!("could not write to .bins file:\n{}", e))?;

        Ok(())
    }

    fn store_bin(&self, from: &Path, force: bool) -> Result<()> {
        let bin_name = from
            .file_name()
            .ok_or_else(|| format_err!("{} isn't a path to a binary", from.display()))?;
        let to = self.layout.bin.join(bin_name);

        if !force && to.exists() {
            bail!(
                "binary {} already exists in the global bin directory",
                bin_name.to_string_lossy().as_ref()
            )
        } else if to.exists() {
            fs::remove_file(&to).with_context(|e| {
                format!("could not remove existing binary {}:\n{}", to.display(), e)
            })?;
        }

        fs::File::create(&to)
            .with_context(|e| format_err!("couldn't create file {}:\n{}", to.display(), e))?;

        let _ = fs::copy(&from, &to).with_context(|e| {
            format_err!(
                "couldn't copy {} to {}:\n{}",
                from.display(),
                to.display(),
                e
            )
        })?;

        Ok(())
    }

    // If bins is empty, it's assumed to mean "delete all binaries"
    pub fn remove_bins(&self, query: &Spec, bins: &[&str]) -> Result<u32> {
        fn contains(sum: &str, query: &Spec) -> bool {
            match (
                &query.name,
                query.resolution.as_ref(),
                query.version.as_ref(),
            ) {
                (_, Some(_), Some(_)) => sum == query.to_string(),
                (name, None, Some(ver)) => {
                    sum.starts_with(&name.to_string()) && sum.ends_with(&ver.to_string())
                }
                _ => sum.starts_with(&query.to_string()),
            }
        };

        let mut c = 0;
        if self.layout.bin.join(".bins").exists() {
            let mut s = String::new();
            let mut f = fs::OpenOptions::new()
                .read(true)
                .open(self.layout.bin.join(".bins"))
                .with_context(|e| format_err!("could not open .bins file:\n{}", e))?;

            f.read_to_string(&mut s)
                .with_context(|e| format_err!("could not read from .bins file:\n{}", e))?;

            let dot: IndexMap<String, String> = toml::from_str(&s)
                .with_context(|e| format_err!("could not deserialize .bins file:\n{}", e))?;

            let (discard, dot): (IndexMap<_, _>, IndexMap<_, _>) =
                dot.into_iter().partition(|(bin, sum)| {
                    (bins.is_empty() || bins.contains(&bin.as_str())) && contains(sum, query)
                });

            let sums = discard.iter().dedup().collect::<Vec<_>>();
            if sums.len() > 1 {
                return Err(format_err!(
                    "spec `{}` is ambiguous between {:?}",
                    query,
                    sums
                ));
            }

            for (bin, _) in discard {
                fs::remove_file(self.layout.bin.join(&bin))
                    .with_context(|e| format_err!("couldn't remove binary {}:\n{}", bin, e))?;
                c += 1;
            }

            drop(f);
            fs::remove_file(self.layout.bin.join(".bins"))
                .with_context(|e| format_err!("could not clear existing .bins file:\n{}", e))?;

            fs::write(
                self.layout.bin.join(".bins"),
                toml::to_string(&dot).unwrap().as_bytes(),
            )
            .with_context(|e| format_err!("could not write to .bins file:\n{}", e))?;
        }

        Ok(c)
    }

    pub fn store_build(&self, from: &Path, hash: &BuildHash) -> Result<Binary> {
        let dest = self.layout.build.join(&hash.0);

        if !dest.exists() {
            fs::create_dir_all(&dest)?;
        }

        let dest = DirLock::acquire(&dest)?;

        clear_dir(dest.path())?;
        copy_dir(from, dest.path(), false)?;

        Ok(Binary::new(dest))
    }

    fn check_build(&self, hash: &BuildHash) -> Option<PathBuf> {
        let path = self.layout.build.join(&hash.0);

        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn get_indices(&self, index_reses: &[DirectRes], eager: bool, offline: bool) -> Indices {
        let mut indices = vec![];
        let mut seen = vec![];
        let mut q: VecDeque<DirectRes> = index_reses.iter().cloned().collect();

        while let Some(index) = q.pop_front() {
            if seen.contains(&index) {
                continue;
            }

            // We special-case a local dir index because `dir` won't exist for it.
            if let DirectRes::Dir { path } = &index {
                let lock = match DirLock::acquire(path) {
                    Ok(dir) => dir,
                    Err(e) => {
                        self.shell.println(
                            style("[warn]").yellow().bold(),
                            format!("Couldn't lock dir index {}: {}", path.display(), e),
                            Verbosity::Quiet,
                        );
                        continue;
                    }
                };

                let ix = Index::from_disk(index.clone(), lock);
                if let Ok(ix) = ix {
                    for dependent in ix.depends().cloned().map(|i| i.res) {
                        q.push_back(dependent);
                    }
                    seen.push(index);
                    indices.push(ix);
                }
                continue;
            }

            let index_path = self.layout.indices.join(Self::get_index_dir(&index));
            let dir = match DirLock::acquire(&index_path) {
                Ok(dir) => dir,
                Err(e) => {
                    self.shell.println(
                        style("[warn]").yellow().bold(),
                        format!("Couldn't lock cached index {}: {}", index, e),
                        Verbosity::Quiet,
                    );
                    continue;
                }
            };

            let res = index.retrieve(&self.client, &dir, eager, |dl_online| {
                if offline && dl_online {
                    return Err(format_err!("Offline mode; can't update indices"));
                }
                self.shell.println(
                    style("Retrieving").cyan(),
                    format!("index {}", &index),
                    Verbosity::Normal,
                );
                Ok(())
            });

            match res {
                Ok(_) => {
                    let ix = Index::from_disk(index.clone(), dir);
                    match ix {
                        Ok(ix) => {
                            for dependent in ix.depends().cloned().map(|i| i.res) {
                                q.push_back(dependent);
                            }
                            seen.push(index);
                            indices.push(ix);
                        }
                        Err(e) => {
                            self.shell.println(
                                style("[warn]").yellow().bold(),
                                format!("Invalid/corrupt index {}: {}", index, e),
                                Verbosity::Quiet,
                            );
                        }
                    }
                }
                Err(e) => {
                    self.shell.println(
                        style("[warn]").yellow().bold(),
                        format!("Couldn't retrieve cache {}: {}", index, e),
                        Verbosity::Quiet,
                    );
                }
            }
        }

        Indices::new(indices)
    }

    fn get_index_dir(loc: &DirectRes) -> String {
        Self::get_source_dir(loc, false)
    }

    /// Returns all of the package hashes available in this cache.
    pub fn cached_packages(&self) -> IndexSet<String> {
        let walker = WalkDir::new(&self.layout.src)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok());

        let mut res = IndexSet::new();

        for dir in walker {
            let fname = dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            if dir.path().join("elba.toml").exists() {
                res.insert(fname);
            }
        }

        res
    }
}

/// Layouts encapsulate the logic behind our directory structure.
#[derive(Debug, Clone)]
pub struct Layout {
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
    pub fn init(&self) -> Result<()> {
        // create_dir_all ignores pre-existing folders
        fs::create_dir_all(&self.bin)?;
        fs::create_dir_all(&self.src)?;
        fs::create_dir_all(&self.build)?;
        fs::create_dir_all(&self.indices)?;
        fs::create_dir_all(&self.tmp)?;

        Ok(())
    }
}

// TODO: Somehow keep track of which targets have been built, so that if a rebuild needs to happen,
//       it doesn't rebuild already built targets, and it reuses the same build dir
/// The Layout of an output directory. This is used either as the `target` directory or one of the
/// folders in a temporary build directory in the global cache.
#[derive(Debug, Clone)]
pub struct OutputLayout {
    lock: Arc<DirLock>,
    pub root: PathBuf,
    pub artifacts: PathBuf,
    pub bin: PathBuf,
    pub docs: PathBuf,
    pub lib: PathBuf,
    pub build: PathBuf,
    pub hash: Option<BuildHash>,
}

impl OutputLayout {
    pub fn new(lock: DirLock) -> Result<Self> {
        let root = lock.path().to_path_buf();

        let layout = OutputLayout {
            lock: Arc::new(lock),
            root: root.clone(),
            artifacts: root.join("artifacts"),
            bin: root.join("bin"),
            docs: root.join("docs"),
            lib: root.join("lib"),
            build: root.join("build"),
            hash: fs::read(root.join("hash"))
                .map(|x| BuildHash(String::from_utf8_lossy(&x).to_string()))
                .ok(),
        };

        // create_dir_all ignores pre-existing folders
        fs::create_dir_all(&layout.root)?;
        fs::create_dir_all(&layout.artifacts)?;
        fs::create_dir_all(&layout.bin)?;
        fs::create_dir_all(&layout.docs)?;
        fs::create_dir_all(&layout.lib)?;
        fs::create_dir_all(&layout.build)?;

        Ok(layout)
    }

    pub fn write_hash(&self, hash: &BuildHash) -> Result<()> {
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
    inner: Arc<SourceInner>,
}

#[derive(Debug)]
struct SourceInner {
    /// The package's manifest
    meta: Manifest,
    /// The original resolution of the package
    res: Resolution,
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
    pub fn from_folder(pkg: &PackageId, path: DirLock, location: DirectRes) -> Result<Self> {
        let toml_path = path.path().join("elba.toml");
        if toml_path.exists() {
            let file = fs::File::open(toml_path).context(format_err!(
                "package {} at {} is missing manifest",
                pkg,
                path.path().display()
            ))?;
            let mut file = BufReader::new(file);
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            if let Some(x) = Manifest::workspace(&contents) {
                if let Some(p) = x.get(pkg.name()) {
                    let lock = DirLock::acquire(&path.path().join(&p.0))?;
                    // We immediately release our lock on the parent folder
                    drop(path);
                    return Source::from_folder(pkg, lock, location);
                }
            }
        }

        let (_, manifest) = find_manifest(path.path(), true, None)?;

        if manifest.name() != pkg.name() {
            bail!(
                "names don't match: {} was declared, but {} was found in elba.toml",
                pkg.name(),
                manifest.name()
            )
        }

        // Creating the hash
        let walker = manifest
            .list_files(path.path(), path.path(), |entry| {
                entry.file_name() != ".git" && entry.file_name() != "target"
            })?
            .filter(valid_file);

        let mut hash = Sha256::new();
        for f in walker {
            let mut file = File::open(f.path())?;
            io::copy(&mut file, &mut hash)?;
        }
        let hash = hex::encode(hash.result());

        Ok(Source {
            inner: Arc::new(SourceInner {
                meta: manifest,
                res: if pkg.resolution().direct().is_some() {
                    location.clone().into()
                } else {
                    pkg.resolution().clone()
                },
                location,
                path,
                hash,
            }),
        })
    }

    pub fn pretty_summary(&self) -> String {
        format!(
            "{} {} ({})",
            self.meta().package.name,
            self.meta().version(),
            self.inner.res,
        )
    }

    pub fn summary(&self) -> String {
        format!(
            "{}@{}|{}",
            self.meta().package.name,
            self.inner.res,
            self.meta().version(),
        )
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
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Binary {
    pub target: Arc<DirLock>,
}

impl Binary {
    pub fn new(target: DirLock) -> Binary {
        Binary {
            target: Arc::new(target),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BuildHash(pub String);

impl BuildHash {
    pub fn new(
        root: &Source,
        sources: &Graph<Source>,
        targets: &Targets,
        ctx: &BuildContext,
        codegen: bool,
    ) -> Self {
        let mut hasher = Sha256::default();
        for (_, src) in sources.sub_tree(sources.find_id(root).unwrap()) {
            hasher.input(&src.hash().as_bytes());
        }

        // Take into account the build context
        if let Ok(ver) = ctx.compiler.version() {
            hasher.input(ver.as_bytes());
        }
        for opt in &ctx.opts {
            hasher.input(opt.as_bytes());
        }
        if codegen {
            hasher.input(ctx.backend.name.as_bytes());
            for opt in &ctx.backend.opts {
                hasher.input(opt.as_bytes());
            }
            hasher.input(
                ctx.backend
                    .extension
                    .as_ref()
                    .map(|x| x.as_bytes())
                    .unwrap_or(&[]),
            );
        }

        // We also hash the targets because if we change the targets for a package, we want to
        // rebuild it
        for t in &targets.0 {
            let bytes: [u8; 5] = t.as_bytes();
            hasher.input(&bytes);
        }
        let hash = hex::encode(hasher.result());

        BuildHash(hash)
    }
}
