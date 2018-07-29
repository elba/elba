use retrieve::Cache;
use std::{fs, path::PathBuf, process::Command};
use util::{errors::Res, lock::DirLock};

// TODO: triple target
#[derive(Debug)]
pub struct BuildContext<'a> {
    pub compiler: Compiler,
    pub cache: &'a Cache,
    // pub config: BuildConfig,
}

// TODO: Verbosity, totality checking
#[derive(Debug)]
pub struct BuildConfig {
    /// In what mode we are compiling
    pub compile_mode: CompileMode,
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

        fs::create_dir_all(&layout.root)?;
        fs::create_dir_all(&layout.bin)?;
        fs::create_dir_all(&layout.lib)?;
        fs::create_dir_all(&layout.build)?;
        fs::create_dir_all(&layout.deps)?;

        Ok(layout)
    }
}
