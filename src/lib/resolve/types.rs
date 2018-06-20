//! Module `resolve/types` defines small miscellaneous types related to dependency resolution.

use err::Error;
use index::{Index, Indices};
use package::{Dep, PackageId};

/// Struct `Queryer` queries indices for a matching package.
pub struct Queryer {
    ixs: Indices,
}

impl Queryer {
    pub fn query(&mut self, dep: Dep) -> Result<PackageId, Error> {
        unimplemented!()
    }
}

/// Struct `ConflictCache` manages a cache of all previous conflicts of packages.
struct ConflictCache {}
