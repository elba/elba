//! Module `resolve/types` provides supplementary types for the `Resolver`.

// `Term` in Pubgrub is just our Dep.

// TODO: Roll our own semver? Or fork semver and add intersections?

use package::{Dep, PackageId};
use std::collections::HashSet;

pub enum DepMatch {
    Satisfies,
    Contradicts,
    Inconclusive,
}

pub struct DepSet(HashSet<Dep>);

impl DepSet {
    pub fn check(&self, dep: &Dep) -> DepMatch {
        unimplemented!()
    }
}

pub struct Incompatibility {
    step: u16,
    deps: HashSet<Dep>,
    /// One possible parent incompatibility which lead to the creation of this one. The `left`
    /// incompatibility is always the first to be created.
    left: Option<Box<Incompatibility>>,
    /// The other possible parent incompatibility of this one. If there's only one parent, this is
    /// `None`.
    right: Option<Box<Incompatibility>>,
}

pub enum IncompatMatch {
    Satisfies,
    Almost,
    Contradicts,
}

impl Incompatibility {
    pub fn new(
        step: u16,
        deps: HashSet<Dep>,
        left: Option<Box<Incompatibility>>,
        right: Option<Box<Incompatibility>>,
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
    Derivation { dep: Dep, cause: u16 },
}
