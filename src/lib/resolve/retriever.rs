use err::{Error, ErrorKind};
use index::Indices;
use package::{lockfile::Lockfile, version::Constraint, PackageId, Summary};
use resolve::types::Incompatibility;
use semver::Version;

// TODO: Patching
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

    // TODO: if subsequent versions of a package have the same dependencies, reflect that in the
    //       incompats
    // TODO: Incompat cache
    /// Returns a `Vec<Incompatibility>` corresponding to the package's dependencies.
    pub fn incompats(&mut self, pkg: &Summary) -> Result<Vec<Incompatibility>, Error> {
        if pkg == &self.root {
            let mut res = vec![];
            for dep in &self.root_deps {
                res.push(Incompatibility::from_dep(pkg.clone(), dep.clone()));
            }
            Ok(res)
        } else if let Some((v, deps)) = self.lockfile.packages.get(pkg.id()) {
            // If the Lockfile has an entry for the package, we use that.
            if v == pkg.version() {
                Ok(deps
                    .clone()
                    .into_iter()
                    .map(|d| {
                        Incompatibility::from_dep(
                            pkg.clone(),
                            (d.id.clone(), d.version.clone().into()),
                        )
                    })
                    .collect())
            } else {
                Ok(self
                    .indices
                    .select(pkg)?
                    .dependencies
                    .iter()
                    .cloned()
                    .map(|d| {
                        let dep = (PackageId::new(d.name, d.index.into()), d.req);
                        Incompatibility::from_dep(pkg.clone(), dep)
                    })
                    .collect())
            }
        } else {
            Ok(self
                .indices
                .select(pkg)?
                .dependencies
                .iter()
                .cloned()
                .map(|d| {
                    let dep = (PackageId::new(d.name, d.index.into()), d.req);
                    Incompatibility::from_dep(pkg.clone(), dep)
                })
                .collect())
        }
    }

    pub fn count_versions(&self, pkg: &PackageId) -> usize {
        self.indices.count_versions(pkg)
    }

    pub fn root(&self) -> &Summary {
        &self.root
    }
}
