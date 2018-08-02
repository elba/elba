//! Utilities for interacting with the Idris compiler

use build::context::BuildContext;
use retrieve::cache::{Binary, OutputLayout};
use std::path::{Path, PathBuf};
use util::{clear_dir, copy_dir, errors::Res};

// CompileInvocation is responsible for dealing with ibc stuff
#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub src: &'a Path,
    pub deps: &'a [&'a Binary],
    pub targets: &'a [PathBuf],
    pub layout: &'a OutputLayout,
}

impl<'a> CompileInvocation<'a> {
    pub fn exec(&self, bcx: &BuildContext) -> Res<()> {
        clear_dir(&self.layout.build)?;
        copy_dir(&self.src, &self.layout.build)?;

        // invoke compiler
        let mut process = bcx.compiler.process();
        process.current_dir(&self.layout.build).arg("--check");

        // Include dependencies
        for binary in self.deps {
            // We assume that the binary has already been compiled
            process.arg("-i").arg(binary.target.path());
        }

        // Add compile units: the individual files that we want to "export" and make available
        for target in self.targets {
            process.arg(target);
        }

        // The moment of truth:
        process.spawn()?;

        Ok(())
    }
}

// If we want to create something with output from a codegen backend (either a library or a binary)
// we look to CodegenInvocation.
#[derive(Debug)]
pub struct CodegenInvocation<'a> {
    pub binary: &'a Path,
    pub output: String,
    pub backend: String,
    pub layout: &'a OutputLayout,
    /// Whether the output should be treated as a binary (false) or artifact files (true)
    pub is_artifact: bool,
}

impl<'a> CodegenInvocation<'a> {
    // TODO: We need more logic here.
    // If we're building an artifact, it should go there instead, with all the relevant info.
    // If we're building a binary, maybe we need some file in the binary directory to keep track of
    // which packages installed what binaries (so that uninstalling is easier)
    pub fn exec(self, bcx: &BuildContext) -> Res<()> {
        // Invoke the compiler.
        // TODO: Canonicalize the build path?
        let mut process = bcx.compiler.process();

        process
            .current_dir(&self.layout.bin)
            .args(&["--codegen", &self.backend])
            .args(&["-o", &self.output])
            .arg(&self.binary);

        process.spawn()?;

        Ok(())
    }
}
