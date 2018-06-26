//! Module `resolve` provides logic for resolving dependency graphs.
//!
//! The dependency resolver in `matic` uses the Pubgrub algorithm for resolving package dependencies,
//! as used by Dart's Pub (https://github.com/dart-lang/pub/blob/master/doc/solver.md). This choice
//! was mainly because the acronyms and stuff in that algorithm sounded cool. Also, it seems to
//! deal with backtracking nicer than Cargo (where the solution is just clone the solver state
//! repeatedly).

pub mod types;

use self::types::{Assignment, AssignmentType, Incompatibility, IncompatMatch};
use err::Error;
use indexmap::IndexMap;
use package::{PackageId, Summary, version::{Constraint, Relation}};
use semver::Version;

// TODO: incompats, derivations, decisions? assignments would just be a log of what happened with
// indices to the other props
pub struct Resolver {
    /// The current step.
    step: u16,
    level: u16,
    assignments: Vec<Assignment>,
    decisions: IndexMap<PackageId, Version>,
    derivations: IndexMap<PackageId, Constraint>,
    incompats: Vec<Incompatibility>,
    incompat_ixs: IndexMap<PackageId, Vec<usize>>,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn solve(&mut self, root: Summary) -> Result<(), Error> {
        let pkgs = indexmap!(root.id().clone() => root.version().clone().into());
        self.incompatibility(pkgs, None, None);

        let mut next = Some(root.id);
        while let Some(n) = next {
            self.propagate(n);
            next = self.choose_pkg_version();
        }

        // Return the solution!
        unimplemented!()
    }

    fn choose_pkg_version(&mut self) -> Option<PackageId> {
        unimplemented!()
    }

    fn propagate(&mut self, pkg: PackageId) {
        let mut changed = indexset!(pkg);

        while let Some(package) = changed.pop() {
            // Yeah, I hate cloning too, but unfortunately it's necessary here
            if let Some(icixs) = self.incompat_ixs.clone().get(&package) {
                'f: for icix in icixs {
                    let res = self.propagate_incompat(*icix);
                    match res {
                        IncompatMatch::Almost(name) => { changed.insert(name); }
                        IncompatMatch::Satisfied => {
                            let root = self.resolve_conflict(*icix);
                            changed.clear();
                            if let IncompatMatch::Almost(name) = self.propagate_incompat(root) {
                                changed.insert(name);
                            } else {
                                unreachable!();
                            }
                            break 'f;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn propagate_incompat(&mut self, icix: usize) -> IncompatMatch {
        // Yes, we're cloning again. I'm sorry.
        let inc = &self.incompats.to_vec()[icix];
        let mut unsatis = None;

        for (pkg, con) in inc.deps() {
            let relation = self.relation(pkg, con);

            if relation == Relation::Disjoint {
                return IncompatMatch::Contradicted;
            } else if relation != Relation::Subset {
                if let None = unsatis {
                    unsatis = Some((pkg, con));
                } else {
                    // We can't deduce anything. This should prolly be "None" instead of
                    // `Contradicted`, but oh well.
                    return IncompatMatch::Contradicted;
                }
            }
        }

        if let Some((pkg, con)) = unsatis {
            let level = self.level;
            self.derivation(pkg.clone(), con.clone(), level, Some(icix));
            return IncompatMatch::Almost(pkg.clone());
        } else {
            return IncompatMatch::Satisfied;
        }
    }

    fn relation(&self, pkg: &PackageId, con: &Constraint) -> Relation {
        if let Some(c) = self.derivations.get(pkg) {
            c.relation(con)
        } else {
            // If we can't find anything, that means it allows all versions!
            // This is different from Constraints, in which not having anything means no solution
            // We don't have Superset, so we use Overlapping (technically true)
            Relation::Overlapping
        }

    }

    fn resolve_conflict(&mut self, inc: usize) -> usize {
        unimplemented!()
    }

    fn decision(&mut self, pkg: PackageId, version: Version) {
        self.level += 1;
        self.assignments.push(Assignment::new(self.step, self.level, AssignmentType::Decision { pkg: pkg.clone(), version: version.clone() } ));
        self.step += 1;
        self.decisions.insert(pkg.clone(), version.clone());
        self.derivations.insert(pkg, version.into());
    }

    fn derivation(&mut self, pkg: PackageId, c: Constraint, level: u16, cause: Option<usize>) {
        if !self.derivations.contains_key(&pkg) {
            self.assignments.push(Assignment::new(self.step, level, AssignmentType::Derivation { pkg: pkg.clone(), constraint: c.clone(), cause }));
            self.step += 1;
            self.derivations.insert(pkg, c);
        } else {
            let (ix, _, old) = self.derivations.get_full_mut(&pkg).unwrap();
            *old = old.intersection(&c);
            self.assignments.push(Assignment::new(self.step, level, AssignmentType::Derivation { pkg, constraint: c, cause }));
            self.step += 1;
        }
    }

    fn incompatibility(&mut self, pkgs: IndexMap<PackageId, Constraint>, left: Option<usize>, right: Option<usize>) {
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
        let level = 0;
        let assignments = vec![];
        let incompats = vec![];
        let incompat_ixs = indexmap!();
        let decisions = indexmap!();
        let derivations = indexmap!();
        Resolver {
            step,
            level,
            assignments,
            incompats,
            incompat_ixs,
            decisions,
            derivations,
        }
    }
}
