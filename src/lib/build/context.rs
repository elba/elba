use std::path::PathBuf;

use util::lock::DirLock;
use util::process_builder::ProcessBuilder;

// TODO: dependency graph, triple target
pub struct BuildContext {
    pub compiler: Compiler,
}

// TODO: Verbosity, Total checking
#[derive(Debug)]
pub struct BuildConfig {}

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
pub struct BuildDir {
    lock: DirLock,
    pub root: PathBuf,
    pub deps: PathBuf,
    pub build: PathBuf,
}

impl BuildDir {
    pub fn new(lock: DirLock) -> Self {
        let root = lock.path().to_path_buf();

        BuildDir {
            lock,
            root: root.clone(),
            deps: root.join("deps"),
            build: root.join("build"),
        }
    }
}
