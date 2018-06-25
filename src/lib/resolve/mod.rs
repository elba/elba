//! Module `resolve` provides logic for resolving dependency graphs.
//!
//! The dependency resolver in `matic` uses the Pubgrub algorithm for resolving package dependencies,
//! as used by Dart's Pub (https://github.com/dart-lang/pub/blob/master/doc/solver.md). This choice
//! was mainly because the acronyms and stuff in that algorithm sounded cool. Also, it seems to
//! deal with backtracking nicer than Cargo (where the solution is just clone the solver state
//! repeatedly).

pub mod types;

use self::types::{Assignment, Incompatibility};
use err::Error;
use index::Index;
use package::{Dep, PackageId, Summary};

pub struct Resolver {
    /// The current step.
    step: u16,
    assignments: Vec<Assignment>,
    incompatibilities: Vec<Incompatibility>,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn solve(&mut self, root: Summary<Dep>) -> Result<(), Error> {
        // First, make a few decisions: choose root package, add incompats for each of its deps
        unimplemented!()
    }

    // TODO: Resolver::{decision, derivation, incompatibility}?
}

impl Default for Resolver {
    fn default() -> Self {
        let step = 1;
        let assignments = vec![];
        let incompatibilities = vec![];
        Resolver {
            step,
            assignments,
            incompatibilities,
        }
    }
}
