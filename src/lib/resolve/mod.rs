//! Module `resolve` provides logic for resolving dependency graphs.
//!
//! The dependency resolver in `matic` uses the Pubgrub algorithm for resolving package dependencies,
//! as used by Dart's Pub (https://github.com/dart-lang/pub/blob/master/doc/solver.md). This choice
//! was mainly because the acronyms and stuff in that algorithm sounded cool. Also, it seems to
//! deal with backtracking nicer than Cargo (where the solution is just clone the solver state
//! repeatedly).

pub mod types;

use self::types::{Assignment, AssignmentType, Incompatibility};
use err::Error;
use indexmap::{IndexMap, IndexSet};
use package::{Dep, Name, PackageId, Summary, version::Constraint};

enum Propagated {
    Satisfied,
    Almost(Name),
    None,
}

// TODO: incompats, derivations, decisions? assignments would just be a log of what happened with
// indices to the other props
pub struct Resolver {
    /// The current step.
    step: u16,
    assignments: Vec<Assignment>,
    incompats: Vec<Incompatibility>,
    incompat_ixs: IndexMap<Name, Vec<usize>>,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn solve(&mut self, root: Summary<Dep>) -> Result<(), Error> {
        let pkgs = indexmap!(root.name().clone() => root.version().clone().into());
        self.incompatibility(pkgs, None, None);

        let mut next = root.id.name;
        // TODO: Loop.

        unimplemented!()
    }

    fn propagate(&mut self, name: Name) {
        let mut changed = indexset!(name);

        while let Some(package) = changed.pop() {
            // Yeah, I hate cloning too, but unfortunately it's necessary here
            if let Some(icixs) = self.incompat_ixs.clone().get(&package) {
                for icix in icixs {
                    let res = self.propagate_incompat(*icix);
                    match res {
                        Propagated::Almost(name) => { changed.insert(name); }
                        Propagated::Satisfied => {
                            let root = self.resolve_conflict(*icix);
                            changed.clear();
                            if let Propagated::Almost(name) = self.propagate_incompat(root) {
                                changed.insert(name);
                            } else {
                                unreachable!();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn propagate_incompat(&mut self, inc: usize) -> Propagated {
        let inc = &self.incompats[inc];
        unimplemented!()
    }

    fn resolve_conflict(&mut self, inc: usize) -> usize {
        unimplemented!()
    }

    fn decision(&mut self, pkg: PackageId, level: u16) {
        self.assignments.push(Assignment::new(self.step, level, AssignmentType::Decision { selected: pkg }));
        self.step += 1;
    }

    fn derivation(&mut self, dep: Dep, level: u16, cause: Option<u16>) {
        self.assignments.push(Assignment::new(self.step, level, AssignmentType::Derivation { dep, cause }));
        self.step += 1;
    }

    fn incompatibility(&mut self, pkgs: IndexMap<Name, Constraint>, left: Option<usize>, right: Option<usize>) {
        let new_ix = self.incompats.len();
        for (n, _) in &pkgs {
            self.incompat_ixs.entry(n.clone()).or_insert_with(Vec::new).push(new_ix);
        }
        self.incompats.push(Incompatibility::new(self.step, pkgs, left, right));
        self.step += 1;
    }
}

impl Default for Resolver {
    fn default() -> Self {
        let step = 1;
        let assignments = vec![];
        let incompats = vec![];
        let incompat_ixs = indexmap!();
        Resolver {
            step,
            assignments,
            incompats,
            incompat_ixs,
        }
    }
}
