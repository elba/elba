use err::{Error, ErrorKind};
use index::Indices;
use package::{lockfile::Lockfile, version::{Constraint, Interval, Range, Relation}, PackageId, Summary};
use resolve::types::{Incompatibility, IncompatibilityCause};
use semver::Version;

// TODO: Patching
// TODO: How to deal with git deps, local file deps from top-level...
/// Retrieves the best packages using both the indices available and a lockfile.
/// By default, prioritizes using a lockfile.
#[derive(Clone, Debug)]
pub struct Retriever {
    root: Summary,
    root_deps: Vec<(PackageId, Constraint)>,
    indices: Indices,
    lockfile: Lockfile,
}

impl Retriever {
    pub fn new(
        root: Summary,
        root_deps: Vec<(PackageId, Constraint)>,
        indices: Indices,
        lockfile: Lockfile,
    ) -> Self {
        Retriever {
            root,
            root_deps,
            indices,
            lockfile,
        }
    }

    /// Chooses the best version of a package given a constraint.
    pub fn best(
        &mut self,
        pkg: &PackageId,
        con: &Constraint,
        minimize: bool,
    ) -> Result<Version, Error> {
        if let Some((v, _)) = self.lockfile.packages.get(pkg) {
            if con.satisfies(v) {
                return Ok(v.clone());
            }
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

    // TODO: For direct deps, give up on widening immediately.
    // TODO: if subsequent versions of a package have the same dependencies, reflect that in the
    //       incompats
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

        let entries = self
            .indices
            .entries(pkg.id())?;

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
                for new_dep in new_deps {
                    if dep.name == new_dep.name && dep.index == new_dep.index {
                        let rel = dep.req.relation(&new_dep.req);
                        if rel == Relation::Equal || rel == Relation::Superset {
                            lower = &new.0;
                        } else {
                            break;
                        }
                    }
                }
            }

            while rix < l - 1 {
                rix += 1;
                let new = entries.get_index(lix).unwrap();
                let new_deps = &new.1.dependencies;
                for new_dep in new_deps {
                    if dep.name == new_dep.name && dep.index == new_dep.index {
                        let rel = dep.req.relation(&new_dep.req);
                        if rel == Relation::Equal || rel == Relation::Superset {
                            upper = &new.0;
                        } else {
                            break;
                        }
                    }
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
