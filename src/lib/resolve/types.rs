//! Module `resolve/types` provides supplementary types for the `Resolver`.

// TODO: Roll our own semver? Or fork semver and add intersections?

use indexmap::IndexMap;
use package::{version::Constraint, PackageId};
use semver::Version;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incompatibility {
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
        deps: IndexMap<PackageId, Constraint>,
        left: Option<usize>,
        right: Option<usize>,
    ) -> Self {
        Incompatibility {
            deps,
            left,
            right,
        }
    }

    pub fn deps(&self) -> &IndexMap<PackageId, Constraint> {
        &self.deps
    }

    pub fn left(&self) -> Option<usize> {
        self.left
    }

    pub fn right(&self) -> Option<usize> {
        self.right
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub step: u16,
    pub level: u16,
    pub ty: AssignmentType,
    pub pkg: PackageId,
}

impl Assignment {
    pub fn new(step: u16, level: u16, pkg: PackageId, ty: AssignmentType) -> Self {
        Assignment { step, level, ty, pkg }
    }

    pub fn ty(&self) -> &AssignmentType {
        &self.ty
    }

    pub fn pkg(&self) -> &PackageId {
        &self.pkg
    }

    pub fn step(&self) -> u16 {
        self.step
    }

    pub fn level(&self) -> u16 {
        self.level
    }

    pub fn cause(&self) -> Option<usize> {
        match &self.ty {
            AssignmentType::Decision { version: _version } => {
                None
            }
            AssignmentType::Derivation { cause, constraint: _constraint } => {
                Some(*cause)
            }
        }
    }

    pub fn constraint(&self) -> Constraint {
        match &self.ty {
            AssignmentType::Decision { version } => {
                version.clone().into()
            }
            AssignmentType::Derivation { constraint, cause: _cause } => {
                constraint.clone()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignmentType {
    Decision { version: Version },
    Derivation { constraint: Constraint, cause: usize },
}
