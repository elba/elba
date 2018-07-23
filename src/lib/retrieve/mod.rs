//! Interfaces for retrieving packages (and information about them) from different sources.
//!
//! Packages can originate from several sources, which complicates getting metadata about them.
//! This module is responsible for smoothing over that process, as well as coordinating the actual
//! retrieval of packages from various different sources (hopefully in parallel).

pub mod cache;

pub use self::cache::Cache;
use failure::Error;
use index::Indices;
use package::{
    resolution::Resolution,
    version::{Constraint, Interval, Range, Relation},
    PackageId, Summary,
};
use resolve::{
    incompat::{Incompatibility, IncompatibilityCause},
    solve::Solve,
};
use semver::Version;
use slog::Logger;
use util::errors::ErrorKind;

// TODO: Patching
// TODO: Multiple root packages so we can support workspaces
/// Retrieves the best packages using both the indices available and a lockfile.
/// By default, prioritizes using a lockfile.
#[derive(Debug)]
pub struct Retriever<'cache> {
    /// The local cache of packages.
    cache: &'cache Cache,
    root: Summary,
    root_deps: Vec<(PackageId, Constraint)>,
    indices: Indices,
    lockfile: Solve,
    pub logger: Logger,
}

impl<'cache> Retriever<'cache> {
    pub fn new(
        plog: &Logger,
        cache: &'cache Cache,
        root: Summary,
        root_deps: Vec<(PackageId, Constraint)>,
        indices: Indices,
        lockfile: Solve,
    ) -> Self {
        let logger = plog.new(o!("root" => root.to_string()));

        Retriever {
            cache,
            root,
            root_deps,
            indices,
            lockfile,
            logger,
        }
    }

    /// Chooses the best version of a package given a constraint.
    pub fn best(
        &mut self,
        pkg: &PackageId,
        con: &Constraint,
        minimize: bool,
    ) -> Result<Version, Error> {
        if let Some(v) = self.lockfile.get_pkg_version(pkg) {
            if con.satisfies(&v) {
                return Ok(v);
            }
        }

        if let Resolution::Direct(loc) = pkg.resolution() {
            return Ok(self
                .cache
                .checkout_source(pkg, loc, None)?
                .meta
                .version()
                .clone());
        }

        if let Resolution::Root = pkg.resolution() {
            return Ok(self.root.version.clone());
        }

        let (mut pre, mut not_pre): (Vec<Version>, Vec<Version>) = self
            .indices
            .entries(pkg)?
            .clone()
            .into_iter()
            .map(|v| v.0)
            .filter(|v| con.satisfies(v))
            .partition(|v| v.is_prerelease());

        if !not_pre.is_empty() {
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
            Err(Error::from(ErrorKind::PackageNotFound))
        }
    }

    // TODO: Incompat cache
    /// Returns a `Vec<Incompatibility>` corresponding to the package's dependencies.
    pub fn incompats(&mut self, pkg: &Summary) -> Result<Vec<Incompatibility>, Error> {
        if pkg == &self.root {
            let mut res = vec![];
            for dep in &self.root_deps {
                res.push(Incompatibility::from_dep(
                    pkg.clone(),
                    (dep.0.clone(), dep.1.complement()),
                ));
            }
            return Ok(res);
        }

        // If this is a DirectRes dep, we ask the cache for info.
        if let Resolution::Direct(loc) = pkg.resolution() {
            let deps = self
                .cache
                .checkout_source(pkg.id(), loc, Some(pkg.version()))?
                .meta
                .deps(&self.cache.def_index, false);
            let mut res = vec![];
            for dep in deps {
                res.push(Incompatibility::from_dep(
                    pkg.clone(),
                    (dep.0.clone(), dep.1.complement()),
                ));
            }
            return Ok(res);
        }

        let entries = self.indices.entries(pkg.id())?;

        let l = entries.len();

        let (ix, ver, start_deps) = entries
            .get_full(pkg.version())
            .map(|x| (x.0, x.1, &x.2.dependencies))
            .ok_or_else(|| ErrorKind::PackageNotFound)?;
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

        Ok(res)
    }

    pub fn count_versions(&self, pkg: &PackageId) -> usize {
        self.indices.count_versions(pkg)
    }

    pub fn root(&self) -> &Summary {
        &self.root
    }
}
