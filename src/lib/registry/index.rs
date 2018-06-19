//! Module `registry/index` defines the format of a registry index, and the operations of an index.

use semver::Version;
use std::collections::BTreeMap;
use std::path::Path;

use err::*;
use package::*;

// TODO: Implement
pub struct Index {
    // TODO: unique id
    hashes: BTreeMap<Name, BTreeMap<Version, Hash>>,
    // TODO: Name, BTreeMap<Version, Summary> ?
    packages: BTreeMap<Name, Vec<Summary>>,
}

impl Index {
    /// Method `Index::save` saves an index to disk.
    pub fn save(&self) -> Res<()> {
        unimplemented!()
    }

    /// Method `Index::load` loads an index from a directory tree.
    pub fn load(&self, path: &Path) -> Res<()> {
        unimplemented!()
    }

    /// Method `Index::unify` unifies multiple indices. Any currently-defined packages are
    /// preferred over new ones.
    pub fn unify(&mut self, other: Index) {
        unimplemented!()
    }
}
