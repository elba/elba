//! Utilities for interacting with the Idris compiler

use build::context::BuildContext;
use itertools::Itertools;
use retrieve::cache::{Binary, OutputLayout};
use std::{
    env,
    path::{Path, PathBuf},
};
use util::{clear_dir, copy_dir, errors::Res};

// CompileInvocation is responsible for dealing with ibc stuff
#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub src: &'a Path,
    pub deps: &'a [&'a Binary],
    pub targets: &'a [PathBuf],
    pub build: &'a Path,
}

impl<'a> CompileInvocation<'a> {
    pub fn exec(&self, bcx: &BuildContext) -> Res<()> {
        clear_dir(&self.build)?;
        copy_dir(&self.src, &self.build)?;

        // invoke compiler
        let mut process = bcx.compiler.process();
        process.current_dir(&self.build).arg("--check");

        // Include dependencies
        for binary in self.deps {
            // We assume that the binary has already been compiled
            process.arg("-i").arg(binary.target.path());
        }

        // We add the arguments passed by the environment variable IDRIS_OPTS at the end so that any
        // conflicting flags will be ignored (idris chooses the earliest flags first)
        if let Ok(val) = env::var("IDRIS_OPTS") {
            process.arg(val);
        }

        // Add compile units: the individual files that we want to "export" and make available
        for target in self.targets {
            process.arg(target);
        }

        // The moment of truth:
        let process = process.output()?;
        // TODO: Better print handling
        if !process.status.success() {
            bail!("{}", String::from_utf8_lossy(&process.stdout))
        }

        Ok(())
    }
}

// If we want to create something with output from a codegen backend (either a library or a binary)
// we look to CodegenInvocation.
#[derive(Debug)]
pub struct CodegenInvocation<'a> {
    pub binary: &'a Path,
    pub output: String,
    pub layout: &'a OutputLayout,
    /// Whether the output should be treated as a binary (false) or artifact files (true)
    pub is_artifact: bool,
}

impl<'a> CodegenInvocation<'a> {
    pub fn exec(self, bcx: &BuildContext) -> Res<()> {
        // Invoke the compiler.
        // TODO: Canonicalize the build path?
        let mut process = bcx.compiler.process();
        let cwd;

        process
            .current_dir(if self.is_artifact {
                cwd = self.layout.artifacts.join(&bcx.backend.name);
                &cwd
            } else {
                &self.layout.bin
            }).args(&["-o", &self.output])
            .args(&[
                if bcx.backend.portable {
                    "--portable-codegen"
                } else {
                    "--codegen"
                },
                &bcx.backend.name,
            ]);

        if !bcx.backend.opts.is_empty() {
            // We put all the cg-opts into a single argument because idk if the Idris compiler
            // allows passing multiple cg-opts in one go
            process
                .arg("--cg-opt")
                .arg(bcx.backend.opts.iter().join(" "));
        }

        // We add the arguments passed by the environment variable IDRIS_OPTS at the end so that any
        // conflicting flags will be ignored (idris chooses the earliest flags first)
        if let Ok(val) = env::var("IDRIS_OPTS") {
            process.arg(val);
        }

        process.arg(&self.binary);

        let process = process.output()?;
        // The Idris compiler is stupid, and won't output a non-zero error code if there's no main
        // function in the file, so we check if stdout is empty instead
        if !process.stdout.is_empty() {
            bail!("{}", String::from_utf8_lossy(&process.stdout))
        }

        Ok(())
    }
}
