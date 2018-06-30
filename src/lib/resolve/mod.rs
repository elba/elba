//! Module `resolve` provides logic for resolving dependency graphs.
//!
//! The dependency resolver in `matic` uses the Pubgrub algorithm for resolving package dependencies,
//! as used by Dart's Pub (https://github.com/dart-lang/pub/blob/master/doc/solver.md). This choice
//! was mainly because the acronyms and stuff in that algorithm sounded cool. Also, it seems to
//! deal with backtracking nicer than Cargo (where the solution is just clone the solver state
//! repeatedly).

mod retriever;
pub mod types;

use self::{
    retriever::Retriever,
    types::{Assignment, AssignmentType, IncompatMatch, Incompatibility, IncompatibilityCause},
};
use err::Error;
use index::Indices;
use indexmap::IndexMap;
use package::{
    lockfile::Lockfile, version::{Constraint, Relation}, PackageId, Summary,
};
use semver::Version;
use std::cmp;

pub struct Resolver {
    /// The current step.
    step: u16,
    level: u16,
    assignments: Vec<Assignment>,
    decisions: IndexMap<PackageId, Version>,
    derivations: IndexMap<PackageId, Constraint>,
    incompats: Vec<Incompatibility>,
    incompat_ixs: IndexMap<PackageId, Vec<usize>>,
    retriever: Retriever,
}

impl Resolver {
    pub fn new(
        root: Summary,
        root_deps: Vec<(PackageId, Constraint)>,
        indices: Indices,
        lockfile: Lockfile,
    ) -> Self {
        let step = 1;
        let level = 0;
        let assignments = vec![];
        let incompats = vec![];
        let incompat_ixs = indexmap!();
        let decisions = indexmap!();
        let derivations = indexmap!();
        let retriever = Retriever::new(root, root_deps, indices, lockfile);
        Resolver {
            step,
            level,
            assignments,
            incompats,
            incompat_ixs,
            decisions,
            derivations,
            retriever,
        }
    }

    pub fn solve(&mut self) -> Result<(), Error> {
        let pkgs = indexmap!(self.retriever.root().id().clone() => self.retriever.root().version().clone().into());
        self.incompatibility(pkgs, IncompatibilityCause::Root);

        let mut next = Some(self.retriever.root().id().clone());
        while let Some(n) = next {
            self.propagate(n);
            next = self.choose_pkg_version();
        }

        // Return the solution!
        unimplemented!()
    }

    fn choose_pkg_version(&mut self) -> Option<PackageId> {
        let unsatisfied = self
            .derivations
            .iter()
            .map(|(d, _)| d)
            .filter(|d| !self.decisions.contains_key(*d))
            .collect::<Vec<_>>();

        if unsatisfied.is_empty() {
            None
        } else {
            unimplemented!()
        }
    }

    fn propagate(&mut self, pkg: PackageId) {
        let mut changed = indexset!(pkg);

        while let Some(package) = changed.pop() {
            // Yeah, I hate cloning too, but unfortunately it's necessary here
            if let Some(icixs) = self.incompat_ixs.clone().get(&package) {
                'f: for icix in icixs {
                    let res = self.propagate_incompat(*icix);
                    match res {
                        IncompatMatch::Almost(name) => {
                            changed.insert(name);
                        }
                        IncompatMatch::Satisfied => {
                            // TODO: Error handling
                            let root = self.resolve_conflict(*icix).unwrap();
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
            self.derivation(pkg.clone(), con.clone(), level, icix);
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
            Relation::Superset
        }
    }

    // This function is basically the only reason why we need NLL; we're doing immutable borrows
    // with satisfier, but mutable ones with backtrack & incompatibility.
    fn resolve_conflict(&mut self, inc: usize) -> Result<usize, Error> {
        let mut new_incompatibility = false;
        let incompats = self.incompats.to_vec();
        let mut i = incompats[inc].clone();

        while !self.is_failure(&i) {
            let mut most_recent_term: Option<(&PackageId, &Constraint)> = None;
            let mut most_recent_satisfier: Option<&Assignment> = None;
            let mut difference: Option<(&PackageId, Constraint)> = None;

            let mut previous_satisfier_level = 1;
            for (pkg, c) in i.deps() {
                // We unwrap here because if this incompatibility is satisfied, it must have
                // been satisfied at some point before...
                let satisfier = self.satisfier(pkg, c).unwrap();

                match most_recent_satisfier {
                    Some(a) => {
                        if a.step() < satisfier.step() {
                            previous_satisfier_level =
                                cmp::max(previous_satisfier_level, a.level());
                            most_recent_term = Some((pkg, c));
                            most_recent_satisfier = Some(satisfier);
                            difference = None;
                        } else {
                            previous_satisfier_level =
                                cmp::max(previous_satisfier_level, satisfier.level());
                        }
                    }
                    None => {
                        most_recent_term = Some((pkg, c));
                        most_recent_satisfier = Some(satisfier);
                    }
                }

                // By this point, most_recent_satisfier and _term will definitely be assigned to.
                let most_recent_satisfier = most_recent_satisfier.unwrap();
                let most_recent_term = most_recent_term.unwrap();
                if most_recent_term == (pkg, c) {
                    difference = {
                        let diff = most_recent_satisfier
                            .constraint()
                            .difference(most_recent_term.1);

                        if diff == Constraint::empty() {
                            None
                        } else {
                            Some((pkg, diff))
                        }
                    };

                    if let Some((pkg, diff)) = difference.clone() {
                        previous_satisfier_level = cmp::max(
                            previous_satisfier_level,
                            self.satisfier(pkg, &diff.complement()).unwrap().level(),
                        );
                    }
                }
            }

            let most_recent_satisfier = most_recent_satisfier.unwrap();
            let most_recent_term = most_recent_term.unwrap();
            if previous_satisfier_level < most_recent_satisfier.level()
                || most_recent_satisfier.cause() == None
            {
                self.backtrack(previous_satisfier_level);
                if new_incompatibility {
                    return Ok(self.incompatibility(i.deps, i.cause));
                } else {
                    return Ok(inc);
                }
            }

            // newterms etc
            let cause = incompats[most_recent_satisfier.cause().unwrap()].clone();
            let mut new_terms: IndexMap<PackageId, Constraint> = IndexMap::new()
                .into_iter()
                .chain(
                    i.deps()
                        .clone()
                        .into_iter()
                        .filter(|t| (&t.0, &t.1) != most_recent_term),
                )
                .chain(
                    cause
                        .deps()
                        .clone()
                        .into_iter()
                        .filter(|t| &t.0 != most_recent_satisfier.pkg()),
                )
                .collect();

            if let Some((pkg, diff)) = difference {
                new_terms.insert(pkg.clone(), diff.complement());
            }

            i = Incompatibility::new(
                new_terms,
                IncompatibilityCause::Derived(inc, most_recent_satisfier.cause().unwrap()),
            );
            new_incompatibility = true;

            // TODO: Some logging here
        }

        // Some error type here
        unimplemented!()
    }

    fn backtrack(&mut self, previous_satisfier_level: u16) {
        let mut packages = indexset!();
        self.level = previous_satisfier_level;

        loop {
            let last = self.assignments.pop().unwrap();
            if last.level() > previous_satisfier_level {
                self.step -= 1;
                packages.insert(last.pkg().clone());
            } else {
                self.assignments.push(last);
                break;
            }
        }

        // Re-compute the constraint for these packages.
        for package in &packages {
            self.decisions.remove(package);
            self.derivations.remove(package);
        }

        let assignments = self.assignments.clone();
        for assignment in assignments {
            if packages.contains(assignment.pkg()) {
                self.register(&assignment);
            }
        }
    }

    fn satisfier(&self, pkg: &PackageId, con: &Constraint) -> Option<&Assignment> {
        let mut assigned_term = Constraint::any();

        for assignment in &self.assignments {
            if assignment.pkg() != pkg {
                continue;
            }

            // TODO: If we want to implement overlapping, it happens here.

            assigned_term = assigned_term.intersection(&assignment.constraint());

            if assigned_term.relation(con) == Relation::Subset {
                return Some(assignment);
            }
        }

        None
    }

    fn is_failure(&self, inc: &Incompatibility) -> bool {
        inc.deps().is_empty()
            || (inc.deps().len() == 1
                && inc.deps().get_index(0).unwrap().0 == self.retriever.root().id())
    }

    fn register(&mut self, a: &Assignment) {
        match a.ty() {
            AssignmentType::Decision { version } => {
                self.decisions.insert(a.pkg().clone(), version.clone());
                self.derivations
                    .insert(a.pkg().clone(), version.clone().into());
            }
            AssignmentType::Derivation {
                cause: _cause,
                constraint,
            } => {
                if !self.derivations.contains_key(a.pkg()) {
                    self.derivations.insert(a.pkg().clone(), constraint.clone());
                } else {
                    let old = self.derivations.get_mut(a.pkg()).unwrap();
                    *old = old.intersection(&constraint);
                }
            }
        }
    }

    fn decision(&mut self, pkg: PackageId, version: Version) {
        self.level += 1;
        let a = Assignment::new(
            self.step,
            self.level,
            pkg,
            AssignmentType::Decision { version: version },
        );
        self.step += 1;
        self.register(&a);
        self.assignments.push(a);
    }

    fn derivation(&mut self, pkg: PackageId, c: Constraint, level: u16, cause: usize) {
        let a = Assignment::new(
            self.step,
            level,
            pkg,
            AssignmentType::Derivation {
                constraint: c,
                cause,
            },
        );
        self.register(&a);
        self.assignments.push(a);
        self.step += 1;
    }

    fn incompatibility(
        &mut self,
        pkgs: IndexMap<PackageId, Constraint>,
        cause: IncompatibilityCause,
    ) -> usize {
        let new_ix = self.incompats.len();
        for (n, _) in &pkgs {
            self.incompat_ixs
                .entry(n.clone())
                .or_insert_with(Vec::new)
                .push(new_ix);
        }
        self.incompats.push(Incompatibility::new(pkgs, cause));
        self.step += 1;

        new_ix
    }
}
