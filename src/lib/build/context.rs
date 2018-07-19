use std::fs;
use std::path::PathBuf;
use std::process::Command;

use failure::Error;
use petgraph::Graph;

use package::Summary;
use util::lock::DirLock;

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
pub struct BuildContext<'a> {
    pub compiler: Compiler,
    pub resolve: &'a Graph<Summary, ()>,
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

    /// Get a process set up to use the found compiler
    pub fn process(&self) -> Command {
        Command::new(&self.path)
    }
}
