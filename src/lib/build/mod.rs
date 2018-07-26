//! Actually building Idris packages.
//!
//! The build process contains two major phases: dependency preparation and target generating.
//!
//! - Dependency preparation:
//!     In this stage, elba retrieves all sources in the resolve tree, and then build each of them
//!     in global cache directory. The works in this phase can be excuted in parallel. When all
//!     dependencies are ready in cache, elba will copy the direct dependency into /target/deps.
//!     Dependency preparation does not necessarily be executed along with target generating and it
//!     could also be used by editors (like rls).
//!
//! - Target generating:
//!     In this stage, Elba builds lib target, binary, docs, benchmarks and tests, only for local package.
//!

pub mod invoke;
pub mod context;
pub mod prepare;

use std::fs;
use std::path::PathBuf;
use util::{errors::Res, lock::DirLock};

/// The general "mode" of what to do
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// Typecheck a target without codegen
    Lib,
    /// Compile and codegen executable(s)
    ///
    /// This subsumes the "Bench" and "Test" modes since those are just compiling and running
    /// executables anyway
    Bin,
    /// Create documentation
    Doc,
}

#[derive(Debug)]
pub struct Layout {
    lock: DirLock,
    pub root: PathBuf,
    pub bin: PathBuf,
    pub lib: PathBuf,
    pub build: PathBuf,
    pub deps: PathBuf,
}

impl Layout {
    pub fn new(lock: DirLock) -> Res<Self> {
        let root = lock.path().to_path_buf();

        let layout = Layout {
            lock,
            root: root.clone(),
            bin: root.join("bin"),
            lib: root.join("lib"),
            build: root.join("build"),
            deps: root.join("deps"),
        };

        fs::create_dir(&layout.root)?;
        fs::create_dir(&layout.bin)?;
        fs::create_dir(&layout.lib)?;
        fs::create_dir(&layout.build)?;
        fs::create_dir(&layout.deps)?;

        Ok(layout)
    }
}
