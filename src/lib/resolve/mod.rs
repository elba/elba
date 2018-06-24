//! Module `resolve` provides logic for resolving dependency graphs.
//!
//! The dependency resolver in `matic` uses the Pubgrub algorithm for resolving package dependencies,
//! as used by Dart's Pub (https://github.com/dart-lang/pub/blob/master/doc/solver.md). This choice
//! was mainly because the acronyms and stuff in that algorithm sounded cool. Also, it seems to
//! deal with backtracking nicer than Cargo (where the solution is just clone the solver state
//! repeatedly).

pub mod types;

use err::Error;
use package::PackageId;
use self::types::{Assignment, Incompatibility};
use std::collections::HashSet;

use index::Index;

pub struct Resolver {
    /// The current step.
    step: u16,
    assignments: Vec<Assignment>,
    incompatibilities: Vec<Incompatibility>,
}

impl Resolver {
    pub fn new() -> Self {
        let step = 1;
        let assignments = vec![];
        let incompatibilities = vec![];
        Resolver { step, assignments, incompatibilities }
    }

    pub fn solve(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}
