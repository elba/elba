//! Assignments for the dependency resolver.

use crate::package::PackageId;
use semver::Version;
use semver_constraints::Constraint;

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
                positive: _positive,
            } => Some(*cause),
        }
    }

    pub fn constraint(&self) -> Constraint {
        match &self.ty {
            AssignmentType::Decision { version } => version.clone().into(),
            AssignmentType::Derivation {
                constraint,
                cause: _cause,
                positive: _positive,
            } => constraint.clone(),
        }
    }

    pub fn is_positive(&self) -> bool {
        match &self.ty {
            AssignmentType::Decision { version: _version } => false,
            AssignmentType::Derivation {
                positive,
                constraint: _constraint,
                cause: _cause,
            } => *positive,
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
        positive: bool,
    },
}
