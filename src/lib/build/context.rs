use crate::{
    retrieve::cache::Cache,
    util::{config::Backend, errors::Res, fmt_output},
};
use failure::{format_err, ResultExt};
use std::{path::PathBuf, process::Command};

// TODO: triple target
#[derive(Debug)]
pub struct BuildContext<'a> {
    pub backend: &'a Backend,
    /// Whether to actually generate code or only check for errors
    pub codegen: bool,
    pub compiler: Compiler,
    /// The global cache to use.
    pub cache: &'a Cache,
    pub threads: u32,
    pub opts: &'a [String],
}

/// Information on the compiler executable
#[derive(Debug)]
pub struct Compiler {
    /// The location of the exe
    pub path: PathBuf,
}

impl Compiler {
    /// Run the compiler at `path` to learn various pieces of information about it.
    pub fn new(name: &str) -> Compiler {
        Compiler {
            path: PathBuf::from(name),
        }
    }

    /// Get a process set up to use the found compiler
    pub fn process(&self) -> Command {
        Command::new(&self.path)
    }

    /// Get the version of the compiler
    pub fn version(&self) -> Res<String> {
        let out = Command::new(&self.path)
            .arg("--version")
            .output()
            .with_context(|e| format!("couldn't invoke version command: {}", e))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(format_err!(
                "couldn't get idris version:\n{}",
                fmt_output(&out)
            ))
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler {
            path: PathBuf::from("idris"),
        }
    }
}
