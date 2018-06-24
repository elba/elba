//! Module `resolve/context` defines the `Context` type, which is passed around for all dependency
//! resolution actions.

use im;
use std::collections::{HashMap, HashSet};

use package::{Dep, PackageId};

/// Struct `ConflictCache` keeps a record of all the dependency conflicts which have already
/// occurred.
pub struct ConflictCache {
    /// The structure of conflicts mirrors that of Cargo and Rust, mapping from a Dep to a Vector
    /// of incompatibilities. An incompatibility itself is a vector of mutually incompatible
    /// locked deps.
    conflicts: HashMap<Dep, Vec<HashSet<PackageId>>>,
    /// The reverse of conflicts. Given a specific package version, returns the dependencies which
    /// are incompatible when this `PackageId` is locked.
    incompats: HashMap<PackageId, HashSet<Dep>>,
}

impl ConflictCache {
    pub fn new() -> Self {
        let conflicts = HashMap::new();
        let incompats = HashMap::new();
        ConflictCache {
            conflicts,
            incompats,
        }
    }

    pub fn insert(&mut self, dep: &Dep, conflicts: &HashSet<PackageId>) {
        let prev = self.conflicts.entry(dep.clone()).or_insert_with(Vec::new);

        if !prev.contains(&conflicts) {
            prev.push(conflicts.clone());
            for c in conflicts {
                self.incompats
                    .entry(c.clone())
                    .or_insert_with(HashSet::new)
                    .insert(dep.clone());
            }
        }
    }
}

// Analogous to `Context`
/// Struct `PackageGraph` encapsulates a graph of dependent packages, which is built up as
/// dependencies are added.
pub struct PackageGraph {}

impl PackageGraph {
    pub fn new() -> Self {
        PackageGraph {}
    }
}

// Analogous to `BacktrackFrame`
/// Struct `Context` keeps track of the entire state of dependency resolution. Contexts are just
/// states of the dependency resolution process which can be switched or reverted to at a later
/// time if resolution fails.
///
/// Contexts are only constructed when there is a possibility of backtracking. If there is only one
/// possible package to choose, there's no point in making a Context which allows for choosing
/// another version of that package (another version which doesn't exist)
pub struct Context {}

impl Context {
    pub fn new() -> Self {
        Context {}
    }
}

pub type States = Vec<Context>;
