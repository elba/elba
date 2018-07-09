//! Module `resolve/types` provides supplementary types for the `Resolver`.

// TODO: Roll our own semver? Or fork semver and add intersections?

use indexmap::IndexMap;
use itertools::Itertools;
use package::Summary;
use package::{version::Constraint, PackageId};
use semver::Version;
use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IncompatibilityCause {
    Dependency,
    Root,
    Unavailable,
    Derived(usize, usize),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Incompatibility {
    pub deps: IndexMap<PackageId, Constraint>,
    pub cause: IncompatibilityCause,
}

#[derive(Clone)]
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

    pub fn derived(&self) -> Option<(usize, usize)> {
        if let IncompatibilityCause::Derived(l, r) = self.cause {
            Some((l, r))
        } else {
            None
        }
    }

    pub fn is_derived(&self) -> bool {
        self.derived().is_some()
    }

    pub fn cause(&self) -> IncompatibilityCause {
        self.cause
    }

    pub fn show(&self) -> String {
        match self.cause {
            IncompatibilityCause::Dependency => {
                assert!(self.deps.len() == 2);
                let depender = self.deps.get_index(0).unwrap();
                let dependee = self.deps.get_index(1).unwrap();
                format!(
                    "{} {} depends on {} {}",
                    depender.0,
                    depender.1,
                    dependee.0,
                    dependee.1.complement()
                )
            }
            IncompatibilityCause::Unavailable => {
                assert!(self.deps.len() == 1);
                let package = self.deps.get_index(0).unwrap();
                format!("no versions of {} match {}", package.0, package.1)
            }
            IncompatibilityCause::Root => format!("version solving failed"),
            IncompatibilityCause::Derived(_, _) => {
                if self.deps.len() == 1 {
                    let package = self.deps.get_index(0).unwrap();
                    format!("{} {} is impossible.", package.0, package.1)
                } else if self.deps.len() == 2 {
                    let p1 = self.deps.get_index(0).unwrap();
                    let p2 = self.deps.get_index(1).unwrap();
                    format!("{} {} requires {} {}", p1.0, p1.1, p2.0, p2.1.complement())
                } else {
                    format!(
                        "one of {} must be false",
                        self.deps
                            .iter()
                            .map(|(k, v)| format!("{} {}", k, v))
                            .join("; ")
                    )
                }
            }
        }
    }

    // TODO: Actually special-case stuff to look nicer.
    pub fn show_combine(
        &self,
        other: &Incompatibility,
        self_linum: Option<u16>,
        other_linum: Option<u16>,
    ) -> String {
        let mut buf = self.show();
        if let Some(l) = self_linum {
            buf.push_str(" (");
            buf.push_str(&l.to_string());
            buf.push(')');
        }
        buf.push_str(" and ");
        buf.push_str(&other.show());
        if let Some(l) = other_linum {
            buf.push_str(" (");
            buf.push_str(&l.to_string());
            buf.push(')');
        }

        buf
    }
}

impl fmt::Debug for Incompatibility {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Incompatibility::{:?}({})",
            self.cause,
            self.deps
                .iter()
                .map(|(k, v)| format!("{} {}", k, v))
                .join("; "),
        )
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
