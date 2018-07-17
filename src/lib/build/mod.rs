//! Actually building Idris packages.

pub mod compiler;
pub mod process_builder;

use std::path::PathBuf;

use self::process_builder::ProcessBuilder;
use util::lock::DirLock;

// TODO: dependency graph, triple target
pub struct BuildContext<'a> {
    compiler: Compiler,
    cfg: &'a BuildConfig,
}

// TODO: Verbosity, Total checking
#[derive(Debug)]
pub struct BuildConfig {
    /// The codegen backend
    pub backend: Option<String>,
    /// In what mode we are compiling
    pub mode: CompileMode,
}

/// The general "mode" of what to do
// TODO: Test, Mkdoc
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// Typecheck a target without codegen
    Check,
    /// Building a target (lib or bin)
    Build,
}

/// Information on the compiler executable
// TODO: Support args and envs
#[derive(Debug)]
pub struct Compiler {
    /// The location of the exe
    pub path: PathBuf,
}

impl Compiler {
    /// Run the compiler at `path` to learn various pieces of information about it.
    // TODO: Actually lookup the complier instead of the hard-coded string.
    pub fn new() -> Compiler {
        Compiler {
            path: PathBuf::from("idris"),
        }
    }

    /// Get a process builder set up to use the found compiler
    pub fn process(&self) -> ProcessBuilder {
        ProcessBuilder::new(&self.path)
    }
}

#[derive(Debug)]
pub struct TargetDir {
    lock: DirLock,
    root: PathBuf,
    deps: PathBuf,
    build: PathBuf,
}

impl TargetDir {
    pub fn new(lock: DirLock) -> Self {
        let root = lock.path().to_path_buf();

        TargetDir {
            lock,
            root: root.clone(),
            deps: root.join("deps"),
            build: root.join("build"),
        }
    }
}
