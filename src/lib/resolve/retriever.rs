use semver::Version;
use err::Error;
use index::Indices;
use package::{lockfile::Lockfile, version::Constraint, PackageId, Summary};
use resolve::types::Incompatibility;

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

    /// Chooses the best version of a package.
    pub fn best(&mut self, pkg: &PackageId, minimize: bool) -> Result<Version, Error> {
        unimplemented!()
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
        } else {
            // If the Lockfile has an entry for the package, we use that.
            if let Some(deps) = self.lockfile.packages.get(pkg) {
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
        }
    }

    pub fn root(&self) -> &Summary {
        &self.root
    }
}
