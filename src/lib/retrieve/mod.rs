//! Interfaces for retrieving packages (and information about them) from different sources.
//!
//! Packages can originate from several sources, which complicates getting metadata about them.
//! This module is responsible for smoothing over that process, as well as coordinating the actual
//! retrieval of packages from various different sources (hopefully in parallel).

pub mod cache;

use std::borrow::Cow;

use console::style;
use failure::{format_err, ResultExt};
use indexmap::{indexmap, IndexMap, IndexSet};
use itertools::Either::{self, Left, Right};
use semver::Version;
use semver_constraints::{Constraint, Interval, Range, Relation};
use slog::{debug, info, o, trace, Logger};

pub use self::cache::{Cache, Source};
use crate::{
    package::{PackageId, Summary},
    remote::{
        resolution::{DirectRes, IndexRes, Resolution},
        Indices, ResolvedEntry,
    },
    resolve::incompat::{Incompatibility, IncompatibilityCause},
    util::{
        error::{Error, Result},
        graph::Graph,
        shell::{Shell, Verbosity},
    },
};

// TODO: Generalized patching and source replacement
// Right now, when using the `--offline` flag, we replace all locations of all index entries with
// locations in the cache instead. We should generalize this process somehow.
// See Cargo for a reference:
// `[patch]`: https://doc.rust-lang.org/cargo/reference/manifest.html#the-patch-section
// Source replacement: https://doc.rust-lang.org/cargo/reference/source-replacement.html

/// Retrieves the best packages using both the indices available and a lockfile.
#[derive(Debug)]
pub struct Retriever<'cache> {
    /// The local cache of packages.
    cache: &'cache Cache,
    root: Summary,
    root_deps: Vec<(PackageId, Constraint)>,
    reses: Vec<DirectRes>,
    indices: Indices,
    indices_set: bool,
    lockfile: Graph<Summary>,
    pub logger: Logger,
    pub ixmap: &'cache IndexMap<String, IndexRes>,
    pub shell: Shell,
    offline_cache: Option<IndexSet<String>>,
    sources: IndexMap<PackageId, Source>,
    pub res_mapping: IndexMap<PackageId, PackageId>,
}

impl<'cache> Retriever<'cache> {
    pub fn new(
        plog: &Logger,
        cache: &'cache Cache,
        root: Summary,
        root_deps: Vec<(PackageId, Constraint)>,
        reses: Either<Vec<DirectRes>, Indices>,
        lockfile: Graph<Summary>,
        ixmap: &'cache IndexMap<String, IndexRes>,
        shell: Shell,
        offline: bool,
    ) -> Self {
        let logger = plog.new(o!("phase" => "retrieve", "root" => root.to_string()));

        let offline_cache = if offline {
            shell.println(
                style("[warn]").yellow().bold(),
                "Offline mode: only cached packages will be used",
                Verbosity::Normal,
            );
            Some(cache.cached_packages())
        } else {
            None
        };

        let (indices, indices_set, reses) = match reses {
            Left(v) => (cache.get_indices(&v, false, offline), false, v),
            Right(e) => (e, true, vec![]),
        };

        Retriever {
            cache,
            root,
            root_deps,
            indices,
            indices_set,
            reses,
            lockfile,
            logger,
            ixmap,
            shell,
            offline_cache,
            sources: indexmap!(),
            res_mapping: indexmap!(),
        }
    }

    /// Loads all of the packages selected in a Solve into the Cache, returning a new graph of all
    /// the Sources.
    ///
    /// This downloads all the packages into the cache. If we wanted to parallelize downloads
    /// later, this is where we'd deal with all the Tokio stuff.
    pub fn retrieve_packages(&mut self, solve: &Graph<Summary>) -> Result<Graph<Source>> {
        // let mut prg = 0;
        // Until pb.println gets added, we can't use progress bars
        // let pb = ProgressBar::new(solve.inner.raw_nodes().len() as u64);
        // pb.set_style(ProgressStyle::default_bar().template("  [-->] {bar} {pos}/{len}"));

        info!(self.logger, "beginning bulk package retrieval");

        let sources = solve.map(|_, sum| {
            let loc = match sum.resolution() {
                Resolution::Direct(direct) => direct.clone(),
                Resolution::Index(_) => self.select(sum).unwrap().into_owned().location,
            };

            if let Some(s) = self.remove(sum.id()) {
                // prg += 1;
                // pb.set_position(prg);
                Ok(s)
            } else {
                let source = self
                    .cache
                    .checkout_source(sum.id(), &loc, false, self.offline_cache.is_some(), || {
                        self.shell.println(
                            style("Retrieving").cyan(),
                            sum.to_string(),
                            Verbosity::Normal,
                        );
                    })
                    .context(format_err!("unable to retrieve package {}", sum))?;
                // prg += 1;
                // pb.set_position(prg);
                Ok(source.1)
            }
        })?;

        // pb.finish_and_clear();
        self.shell.println(
            style("Cached").dim(),
            format!("packages in {}", self.cache.layout.src.display()),
            Verbosity::Verbose,
        );

        info!(self.logger, "retrieve successful"; "cache" => self.cache.layout.src.display());

        Ok(sources)
    }

    /// Chooses the best version of a package given a constraint.
    pub fn best(&mut self, pkg: &PackageId, con: &Constraint, minimize: bool) -> Result<Version> {
        // With stuff from lockfiles, we try to retrieve whatever version was specified in the
        // lockfile. However, if it fails, we don't want to error out; we want to try to find
        // the best version we can otherwise.
        let locked = self.lockfile.find_by(|sum| sum.id.lowkey_eq(pkg));

        if let Some(lp) = locked {
            debug!(
                self.logger, "found locked pkg";
                "locked" => lp.to_string(),
                "given" => pkg.to_string(),
                "constraint" => con.to_string()
            );
            let v = &lp.version;
            if con.satisfies(&v) {
                if let Resolution::Direct(dir) = lp.resolution() {
                    debug!(
                        self.logger, "good locked pkg";
                        "given" => pkg.to_string(),
                        "locked" => lp.to_string(),
                        "type" => "direct"
                    );
                    let dir = dir.clone();
                    if let Ok(src) = self.direct_checkout(pkg, Some(&dir), false) {
                        return Ok(src.meta().version().clone());
                    }
                } else {
                    debug!(
                        self.logger, "good locked pkg";
                        "given" => pkg.to_string(),
                        "locked" => lp.to_string(),
                        "type" => "index"
                    );
                    let v = v.clone();
                    return self
                        .select(&Summary::new(pkg.clone(), v))
                        .map(|e| e.into_owned().version);
                };
            }
        }

        if pkg.resolution().direct().is_some() {
            debug!(
                self.logger, "new chosen pkg";
                "given" => pkg.to_string(),
                "type" => "direct"
            );
            return Ok(self
                .direct_checkout(pkg, None, true)?
                .meta()
                .version()
                .clone());
        }

        self.get_indices();

        let (mut pre, mut not_pre): (Vec<Version>, Vec<Version>) = self
            .entries(pkg)?
            .into_owned()
            .into_iter()
            .map(|v| v.0)
            .filter(|v| con.satisfies(v))
            .partition(|v| v.is_prerelease());

        let res = if !not_pre.is_empty() {
            if !minimize {
                Ok(not_pre.pop().unwrap())
            } else {
                Ok(not_pre.remove(0))
            }
        } else if !pre.is_empty() {
            if !minimize {
                Ok(pre.pop().unwrap())
            } else {
                Ok(pre.remove(0))
            }
        } else {
            Err(Error::from(Error::PackageNotFound))
        };

        debug!(
            self.logger, "new chosen pkg";
            "given" => pkg.to_string(),
            "version" => res.as_ref().map(|x| x.to_string()).unwrap_or_else(|_| "err".to_string()),
            "type" => "index"
        );

        res.map_err(Into::into)
    }

    /// Returns a `Vec<Incompatibility>` corresponding to the package's dependencies.
    pub fn incompats(
        &mut self,
        pkg: &Summary,
        parent_pkg: &PackageId,
    ) -> Result<Vec<Incompatibility>> {
        if pkg == &self.root {
            let mut res = vec![];
            for dep in &self.root_deps {
                res.push(Incompatibility::from_dep(
                    pkg.clone(),
                    (dep.0.clone(), dep.1.complement()),
                ));
            }
            trace!(
                self.logger, "pkg incompats";
                "from" => "root",
                "pkg" => pkg.to_string(),
                "incompats" => format!("{:?}", res),
            );
            return Ok(res);
        }

        // If this is a DirectRes dep, we ask the cache for info.
        if pkg.resolution().direct().is_some() {
            let ixmap = self.ixmap.clone();
            let deps = self
                .direct_checkout(pkg.id(), None, false)?
                .meta()
                .deps(&ixmap, parent_pkg, false)?;

            let mut res = vec![];
            for dep in deps {
                res.push(Incompatibility::from_dep(
                    pkg.clone(),
                    (dep.0.clone(), dep.1.complement()),
                ));
            }
            trace!(
                self.logger, "pkg incompats";
                "from" => "direct",
                "pkg" => pkg.to_string(),
                "incompats" => format!("{:?}", res),
            );
            return Ok(res);
        }

        let entries = self.entries(pkg.id())?;
        let l = entries.len();

        let (ix, ver, start_deps) = entries
            .get_full(pkg.version())
            .map(|x| (x.0, x.1, &x.2.dependencies))
            .ok_or_else(|| Error::PackageNotFound)?;
        let mut res = vec![];

        for dep in start_deps {
            let mut lix = ix;
            let mut lower = ver;
            let mut rix = ix;
            let mut upper = ver;

            while lix > 0 {
                lix -= 1;
                let new = entries.get_index(lix).unwrap();
                let new_deps = &new.1.dependencies;
                let mut seen = false;
                for new_dep in new_deps {
                    if dep.name == new_dep.name && dep.index == new_dep.index {
                        let rel = dep.req.relation(&new_dep.req);
                        if rel == Relation::Equal || rel == Relation::Superset {
                            seen = true;
                            lower = &new.0;
                        } else {
                            seen = false;
                        }
                    }
                }
                if !seen {
                    lix += 1;
                    break;
                }
            }

            while rix < l - 1 {
                rix += 1;
                let new = entries.get_index(rix).unwrap();
                let new_deps = &new.1.dependencies;
                let mut seen = false;
                for new_dep in new_deps {
                    if dep.name == new_dep.name && dep.index == new_dep.index {
                        let rel = dep.req.relation(&new_dep.req);
                        if rel == Relation::Equal || rel == Relation::Superset {
                            seen = true;
                            upper = &new.0;
                        } else {
                            seen = false;
                        }
                    }
                }
                if !seen {
                    rix -= 1;
                    break;
                }
            }

            let nl = if lix == 0 && rix == l - 1 {
                Interval::Unbounded
            } else {
                Interval::Closed(lower.clone(), false)
            };

            let nu = if lix == 0 && rix == l - 1 {
                Interval::Unbounded
            } else {
                Interval::Closed(upper.clone(), false)
            };

            let dep_pkg = PackageId::new(dep.name.clone(), dep.index.clone().into());

            let cs = indexmap!(
                pkg.id().clone() => Range::new(nl, nu).unwrap().into(),
                dep_pkg => dep.req.complement(),
            );

            res.push(Incompatibility::new(cs, IncompatibilityCause::Dependency))
        }

        trace!(
            self.logger, "pkg incompats";
            "from" => "index",
            "pkg" => pkg.to_string(),
            "incompats" => format!("{:?}", res),
        );

        Ok(res)
    }

    pub fn count_versions(&self, pkg: &PackageId) -> usize {
        if let Some(cache) = self.offline_cache.as_ref() {
            self.indices
                .cache
                .get(pkg)
                .map(|x| {
                    x.iter()
                        .filter(|(_, e)| {
                            let hash = Cache::get_source_dir(&e.location, false);
                            cache.contains(&hash)
                        })
                        .count()
                })
                .unwrap_or(0)
        } else {
            self.indices.count_versions(pkg)
        }
    }

    pub fn select(&mut self, sum: &Summary) -> Result<Cow<ResolvedEntry>> {
        if let Some(cache) = self.offline_cache.as_ref() {
            let selected = self.indices.select(sum)?;
            let hash = Cache::get_source_dir(&selected.location, false);
            if cache.contains(&hash) {
                let mut selected = selected.clone();
                selected.location = DirectRes::Dir {
                    path: self.cache.layout.src.join(&hash),
                };
                Ok(Cow::Owned(selected))
            } else {
                Err(Error::PackageNotFound)?
            }
        } else {
            let res = self.indices.select(sum);
            if res.is_err() && !self.indices_set {
                drop(res);
                self.get_indices();
                self.select(sum)
            } else {
                Ok(Cow::Borrowed(self.indices.select(sum)?))
            }
        }
    }

    pub fn entries(&mut self, pkg: &PackageId) -> Result<Cow<IndexMap<Version, ResolvedEntry>>> {
        if let Some(cache) = self.offline_cache.as_ref() {
            let mut entries = self.indices.entries(pkg)?.clone();
            for (_, e) in entries.iter_mut() {
                let hash = Cache::get_source_dir(&e.location, false);
                if cache.contains(&hash) {
                    e.location = DirectRes::Dir {
                        path: self.cache.layout.src.join(&hash),
                    };
                } else {
                    return Err(Error::PackageNotFound)?;
                }
            }

            Ok(Cow::Owned(entries))
        } else {
            let res = self.indices.entries(pkg);
            if res.is_err() && !self.indices_set {
                drop(res);
                self.get_indices();
                self.entries(pkg)
            } else {
                Ok(Cow::Borrowed(self.indices.entries(pkg)?))
            }
        }
    }

    pub fn root(&self) -> &Summary {
        &self.root
    }

    pub fn direct_checkout(
        &mut self,
        pkg: &PackageId,
        og: Option<&DirectRes>,
        eager: bool,
    ) -> Result<&Source> {
        trace!(
            self.logger, "direct checkout";
            "pkg" => pkg.to_string(),
            "og" => format!("{:?}", og),
            "eager" => eager.to_string()
        );
        if self.res_mapping.contains_key(pkg) {
            Ok(&self.sources[&self.res_mapping[pkg]])
        } else if self.sources.contains_key(pkg) {
            Ok(&self.sources[pkg])
        } else {
            let loc = og.unwrap_or_else(|| pkg.resolution().direct().unwrap());
            let (new_res, s) = self.cache.checkout_source(
                &pkg,
                &loc,
                eager,
                self.offline_cache.is_some(),
                || {
                    self.shell.println(
                        style("Retrieving").cyan(),
                        format!("{} ({})", pkg.name(), pkg.resolution()),
                        Verbosity::Normal,
                    );
                },
            )?;

            let res = if let Some(delta) = new_res {
                delta
            } else {
                loc.clone()
            };

            let new_id = PackageId::new(pkg.name().clone(), res.into());

            self.res_mapping.insert(pkg.clone(), new_id.clone());
            self.sources.insert(new_id.clone(), s);
            Ok(&self.sources[&new_id])
        }
    }

    pub fn remove(&mut self, pkg: &PackageId) -> Option<Source> {
        if self.res_mapping.contains_key(pkg) {
            self.sources.remove(&self.res_mapping[pkg])
        } else {
            self.sources.remove(pkg)
        }
    }

    fn get_indices(&mut self) {
        if !self.indices_set {
            debug!(self.logger, "updating indices eagerly");
            self.indices = self
                .cache
                .get_indices(&self.reses, true, self.offline_cache.is_some());
            self.indices_set = true;
            self.shell.println(
                style("Cached").dim(),
                format!("indices at {}", self.cache.layout.indices.display()),
                Verbosity::Verbose,
            );
        }
    }
}
