//! Module `resolve/types` provides supplementary types for the `Resolver`.

// TODO: Roll our own semver? Or fork semver and add intersections?

use indexmap::IndexMap;
use package::{version::Constraint, PackageId};
use semver::Version;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incompatibility {
    step: u16,
    deps: IndexMap<PackageId, Constraint>,
    /// One possible parent incompatibility which lead to the creation of this one. The `left`
    /// incompatibility is always the first to be created.
    left: Option<usize>,
    /// The other possible parent incompatibility of this one. If there's only one parent, this is
    /// `None`.
    right: Option<usize>,
}

pub enum IncompatMatch {
    Satisfied,
    Almost(PackageId),
    Contradicted,
}

impl Incompatibility {
    pub fn new(
        step: u16,
        deps: IndexMap<PackageId, Constraint>,
        left: Option<usize>,
        right: Option<usize>,
    ) -> Self {
        Incompatibility {
            step,
            deps,
            left,
            right,
        }
    }

    pub fn deps(&self) -> &IndexMap<PackageId, Constraint> {
        &self.deps
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    step: u16,
    level: u16,
    ty: AssignmentType,
}

impl Assignment {
    pub fn new(step: u16, level: u16, ty: AssignmentType) -> Self {
        Assignment { step, level, ty }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignmentType {
    Decision { pkg: PackageId, version: Version },
    Derivation { pkg: PackageId, constraint: Constraint, cause: Option<usize> },
}
