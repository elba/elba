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

use err::{Error, ErrorKind};
use failure::ResultExt;
use flate2::read::GzDecoder;
use indexmap::IndexMap;
use package::{
    manifest::{DepReq, Manifest},
    resolution::{DirectRes, IndexRes, Resolution},
    version::Constraint,
    Name, PackageId
};
use reqwest::Client;
use semver::Version;
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{prelude::*, BufReader},
    path::PathBuf,
    str::FromStr,
};
use symlink::symlink_dir;
use tar::Archive;

/// Utility function to turn an Sha2 Hash into a nice hex format
fn hexify_hash(hash: &[u8]) -> String {
    let mut s = String::new();
    for byte in hash {
        let p = format!("{:02x}", byte);
        s.push_str(&p);
    }
    s
}

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
/// The src and build folders contain one folder for every group on disk. Each of those has
/// all the packages.
///
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

    /// Retrieve the metadata of a package, loading it into the cache if necessary. This is used
    /// for non-index dependencies.
    pub fn metadata(&self, pkg: &PackageId, v: Option<&Version>, loc: &DirectRes) -> Result<CacheMeta, Error> {
        let p = self.load(pkg, v, loc)?;
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
    /// Returns a future pointing to the path to a downloaded (and potentially extracted, if it's a
    /// tarball) package.
    ///
    /// If the package has been cached, this function does no I/O. If it hasn't, it goes wherever
    /// it needs to in order to retrieve the package.
    ///
    /// If the package has a direct resolution of a local file directory, this just symlinks the
    /// package into the directory of the cache.
    pub fn load(&self, pkg: &PackageId, v: Option<&Version>, loc: &DirectRes) -> Result<PathBuf, Error> {
        if let Some(path) = self.check(pkg, v) {
            Ok(path)
        } else {
            match loc {
                DirectRes::Tar { url, cksum } => match url.scheme() {
                    "http" | "https" => self.client
                        .get(url.clone())
                        .send()
                        .map_err(|_| Error::from(ErrorKind::CannotDownload))
                        .and_then(|mut r| {
                            let mut buf: Vec<u8> = vec![];
                            r.copy_to(&mut buf).context(ErrorKind::CannotDownload)?;

                            let hash = hexify_hash(Sha512::digest(&buf[..]).as_slice());
                            if let Some(cksum) = cksum {
                                if &cksum.hash == &hash {
                                    return Err(ErrorKind::Checksum)?;
                                }
                            }

                            let archive = BufReader::new(&buf[..]);
                            let archive = GzDecoder::new(archive);
                            let mut archive = Archive::new(archive);

                            let mut p = self.location.clone();
                            p.push("src");
                            p.push(Self::get_dir(pkg, v));

                            archive.unpack(&p).context(ErrorKind::CannotDownload)?;

                            Ok(p)
                        }),
                    "file" => {
                        let mut p = self.location.clone();
                        p.push("src");
                        p.push(Self::get_dir(pkg, v));

                        let mut archive = fs::File::open(p).context(ErrorKind::CannotDownload)?;

                        let hash = hexify_hash(
                            Sha512::digest_reader(&mut archive)
                                .context(ErrorKind::CannotDownload)?
                                .as_slice(),
                        );

                        if let Some(cksum) = cksum {
                            if &cksum.hash == &hash {
                                return Err(ErrorKind::Checksum)?;
                            }
                        }

                        let archive = BufReader::new(archive);
                        let archive = GzDecoder::new(archive);
                        let mut archive = Archive::new(archive);

                        let mut p = self.location.clone();
                        p.push("src");
                        p.push(Self::get_dir(pkg, v));

                        archive.unpack(&p).context(ErrorKind::CannotDownload)?;

                        // TODO: Checksum

                        Ok(p)
                    }
                    _ => Err(Error::from(ErrorKind::CannotDownload)),
                },
                // TODO: Workspaces.
                DirectRes::Git { repo, tag } => unimplemented!(),
                DirectRes::Dir { url } => {
                    // If this package is located on disk, we just create a symlink into the cache
                    // directory.
                    let src = url.to_file_path().unwrap();
                    let mut dst = self.location.clone();
                    dst.push("src");
                    dst.push(Self::get_dir(pkg, v));
                    // We don't try to copy-paste at all. If we can't symlink, we just give up.
                    symlink_dir(src, &dst).context(ErrorKind::CannotDownload)?;

                    Ok(dst)
                }
            }
        }
    }

    /// Check if package is downloaded and in the cache. If so, returns the path of the cached
    /// package.
    pub fn check(&self, pkg: &PackageId, v: Option<&Version>) -> Option<PathBuf> {
        let mut path = self.location.clone();
        path.push("src");
        path.push(Self::get_dir(pkg, v));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    // TODO: {cabal new-build/nix}-style identifiers for downloaded packages. Instead of packages
    // being identified by their Summary, they're identified by a hash which includes Summary and
    // other stuff (maybe dependencies of the package, maybe features, maybe the code itself).
    //
    // e.g. a cached package directory used to be `group/test@test#1.0.0/`.
    // now it'll be `test-a12f312f12edw21w/`
    //
    // For cabal new-build, this is specifically relevant for globally caching builds, because the
    // deps can change output.
    /// Gets the corresponding directory of a package. We need this because for packages which have
    /// no associated version (i.e. git and local dependencies, where the constraints are inherent
    /// in the resolution itself), we ignore a version specifier.
    fn get_dir(pkg: &PackageId, v: Option<&Version>) -> String {
        if let Resolution::Direct(_) = pkg.resolution() {
            format!("{}", pkg)
        } else {
            format!("{}#{}", pkg, v.unwrap())
        }
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
