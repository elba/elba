//! Module `resolve/types` provides supplementary types for the `Resolver`.

// TODO: Roll our own semver? Or fork semver and add intersections?

use indexmap::IndexMap;
use package::Summary;
use package::{version::Constraint, PackageId};
use semver::Version;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IncompatibilityCause {
    Dependency,
    NoVersions,
    Root,
    Unavailable,
    Derived(usize, usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incompatibility {
    pub deps: IndexMap<PackageId, Constraint>,
    pub cause: IncompatibilityCause,
}

pub enum IncompatMatch {
    Satisfied,
    Almost(PackageId),
    Contradicted,
}

impl Incompatibility {
    pub fn new(deps: IndexMap<PackageId, Constraint>, cause: IncompatibilityCause) -> Self {
        Incompatibility { deps, cause }
    }

    pub fn from_dep(a: Summary, b: (PackageId, Constraint)) -> Self {
        let m = indexmap!(
            a.id => a.version.into(),
            b.0 => b.1,
        );

        Incompatibility::new(m, IncompatibilityCause::Dependency)
    }

    pub fn deps(&self) -> &IndexMap<PackageId, Constraint> {
        &self.deps
    }

    pub fn left(&self) -> Option<usize> {
        if let IncompatibilityCause::Derived(l, _) = self.cause {
            Some(l)
        } else {
            None
        }
    }

    pub fn right(&self) -> Option<usize> {
        if let IncompatibilityCause::Derived(_, r) = self.cause {
            Some(r)
        } else {
            None
        }
    }

    pub fn cause(&self) -> IncompatibilityCause {
        self.cause
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
        Assignment {
            step,
            level,
            ty,
            pkg,
        }
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
            AssignmentType::Decision { version: _version } => None,
            AssignmentType::Derivation {
                cause,
                constraint: _constraint,
            } => Some(*cause),
        }
    }

    pub fn constraint(&self) -> Constraint {
        match &self.ty {
            AssignmentType::Decision { version } => version.clone().into(),
            AssignmentType::Derivation {
                constraint,
                cause: _cause,
            } => constraint.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignmentType {
    Decision {
        version: Version,
    },
    Derivation {
        constraint: Constraint,
        cause: usize,
    },
}
