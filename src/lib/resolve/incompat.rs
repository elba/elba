//! Incompatibilities for the dependency resolver.

use crate::package::{PackageId, Summary};
use indexmap::{indexmap, IndexMap};
use itertools::Itertools;
use semver_constraints::Constraint;
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
                format!("{} {} is unavailable", package.0, package.1)
            }
            IncompatibilityCause::Root => "the root package was chosen".to_string(),
            IncompatibilityCause::Derived(_, _) => {
                if self.deps.len() == 1 {
                    let package = self.deps.get_index(0).unwrap();
                    format!("{} {} is impossible", package.0, package.1)
                } else if self.deps.len() == 2 {
                    let p1 = self.deps.get_index(0).unwrap();
                    let p2 = self.deps.get_index(1).unwrap();
                    format!("{} {} is incompatible with {} {}", p1.0, p1.1, p2.0, p2.1)
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
        if let Some(b) = self.show_combine_same(other, self_linum) {
            return b;
        }

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

    fn show_combine_same(
        &self,
        other: &Incompatibility,
        self_linum: Option<u16>,
    ) -> Option<String> {
        if self == other {
            let mut buf = self.show();
            if let Some(l) = self_linum {
                buf.push_str(" (");
                buf.push_str(&l.to_string());
                buf.push(')');
            }
            Some(buf)
        } else {
            None
        }
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
