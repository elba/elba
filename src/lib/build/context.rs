use retrieve::cache::{Cache, Source};
use std::{path::PathBuf, process::Command};
use util::graph::Graph;

/// An unit that elba knows how to build it
// #[derive(Debug)]
// pub struct Unit<'a> {
//     summary: Summary,
//     resolve: &'a Graph<Summary, ()>,
// }

// impl<'a> Unit<'a> {
//     pub fn new(summary: Summary, bcx: BuildContext<'a>) -> Self {
//         Unit {
//             summary,
//             resolve: bcx.resolve
//         }
//     }
// }

// TODO: triple target
#[derive(Debug)]
pub struct BuildContext<'a> {
    pub compiler: Compiler,
    pub resolve: Graph<Source>,
    pub cache: &'a Cache,
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
    // TODO: Actually lookup the compiler instead of the hard-coded string.
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
