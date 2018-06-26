//! Module `resolve/types` provides supplementary types for the `Resolver`.

// `Term` in Pubgrub is just our Dep.

// TODO: Roll our own semver? Or fork semver and add intersections?

use indexmap::IndexMap;
use package::{Dep, Name, version::Constraint, PackageId};

pub enum DepMatch {
    Satisfies,
    Contradicts,
    Inconclusive,
}

pub struct DepSet(IndexMap<Name, Constraint>);

impl DepSet {
    pub fn check(&self, dep: &Dep) -> DepMatch {
        unimplemented!()
    }
}

pub struct Incompatibility {
    step: u16,
    deps: IndexMap<Name, Constraint>,
    /// One possible parent incompatibility which lead to the creation of this one. The `left`
    /// incompatibility is always the first to be created.
    left: Option<usize>,
    /// The other possible parent incompatibility of this one. If there's only one parent, this is
    /// `None`.
    right: Option<usize>,
}

pub enum IncompatMatch {
    Satisfies,
    Almost,
    Contradicts,
}

impl Incompatibility {
    pub fn new(
        step: u16,
        deps: IndexMap<Name, Constraint>,
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

    pub fn check(&self, dep: &Dep) -> IncompatMatch {
        unimplemented!()
    }
}

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

pub enum AssignmentType {
    Decision { selected: PackageId },
    Derivation { dep: Dep, cause: Option<u16> },
}
