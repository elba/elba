use crate::{
    retrieve::cache::Cache,
    util::{config::Backend, error::Result, fmt_output},
};
use failure::{format_err, ResultExt};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone)]
pub struct BuildContext {
    pub backend: Backend,
    /// Whether to actually generate code or only check for errors
    pub codegen: bool,
    pub compiler: Compiler,
    /// The global cache to use.
    pub cache: Cache,
    pub threads: u32,
    pub opts: Vec<String>,
}

/// Information on the compiler executable
#[derive(Debug, Clone)]
pub struct Compiler {
    /// The location of the exe
    path: PathBuf,
    flavor: CompilerFlavor,
}

impl Compiler {
    /// Run the compiler at `path` to learn various pieces of information about it.
    pub fn new(name: &str) -> Result<Compiler> {
        let c = Compiler {
            path: PathBuf::from(name),
            flavor: CompilerFlavor::Idris1,
        };

        let flavor = if c.version()?.starts_with("Idris 2") {
            CompilerFlavor::Idris2
        } else {
            CompilerFlavor::Idris1
        };

        Ok(Compiler { flavor, ..c })
    }

    /// Get a process set up to use the found compiler
    pub fn process(&self) -> Command {
        Command::new(&self.path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn flavor(&self) -> CompilerFlavor {
        self.flavor
    }

    /// Get the version of the compiler
    pub fn version(&self) -> Result<String> {
        let out = Command::new(&self.path)
            .arg("--version")
            .output()
            .with_context(|e| format!("couldn't retrieve Idris compiler version: {}", e))?;
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
            flavor: CompilerFlavor::Idris1,
        }
    }
}

/// The type of compiler we're dealing with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompilerFlavor {
    Idris1,
    Idris2,
}

impl CompilerFlavor {
    pub fn is_idris1(self) -> bool {
        match self {
            CompilerFlavor::Idris1 => true,
            _ => false,
        }
    }

    pub fn is_idris2(self) -> bool {
        match self {
            CompilerFlavor::Idris2 => true,
            _ => false,
        }
    }
}
