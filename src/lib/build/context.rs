use retrieve::cache::Cache;
use std::{path::PathBuf, process::Command};
use util::config::Backend;

// TODO: triple target
#[derive(Debug)]
pub struct BuildContext<'a> {
    pub backend: &'a Backend,
    pub compiler: Compiler,
    /// The global cache to use.
    pub cache: &'a Cache,
    pub threads: u8,
    pub config: BuildConfig,
}

// TODO: Verbosity, totality checking
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
    // TODO: Actually look up the compiler instead of the hard-coded string.
    pub fn new() -> Compiler {
        Compiler::default()
    }

    /// Get a process set up to use the found compiler
    pub fn process(&self) -> Command {
        Command::new(&self.path)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler {
            path: PathBuf::from("idris"),
        }
    }
}
