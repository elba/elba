//! Utilities for interacting with the Idris compiler

use build::context::BuildContext;
use itertools::Itertools;
use retrieve::cache::{Binary, OutputLayout};
use std::{
    path::{Path, PathBuf},
    process::Output,
};
use util::{clear_dir, copy_dir, errors::Res, fmt_output};

// CompileInvocation is responsible for dealing with ibc stuff
#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub src: &'a Path,
    pub deps: &'a [&'a Binary],
    pub targets: &'a [PathBuf],
    pub build: &'a Path,
    pub args: &'a [String],
}

impl<'a> CompileInvocation<'a> {
    pub fn exec(&self, bcx: &BuildContext) -> Res<Output> {
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

        process.args(self.args);

        // Add compile units: the individual files that we want to "export" and make available
        for target in self.targets {
            process.arg(target);
        }

        // The moment of truth:
        let res = process.output()?;
        if !res.status.success() {
            bail!("[cmd] {:#?}\n{}", process, fmt_output(&res))
        }

        Ok(res)
    }
}

// If we want to create something with output from a codegen backend (either a library or a binary)
// we look to CodegenInvocation.
#[derive(Debug)]
pub struct CodegenInvocation<'a> {
    pub binary: &'a [PathBuf],
    pub output: &'a str,
    pub layout: &'a OutputLayout,
    /// Whether the output should be treated as a binary (false) or artifact files (true)
    pub is_artifact: bool,
    pub args: &'a [String],
}

impl<'a> CodegenInvocation<'a> {
    pub fn exec(self, bcx: &BuildContext) -> Res<Output> {
        // Invoke the compiler.
        // TODO: Canonicalize the build path?
        let mut process = bcx.compiler.process();
        let cwd;

        if self.is_artifact {
            process.arg("--interface");
        }

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
            process
                .arg("--cg-opt")
                .arg(bcx.backend.opts.iter().join(" "));
        }

        process.args(self.args);

        for bin in self.binary {
            process.arg(bin);
        }

        let res = process.output()?;
        // The Idris compiler is stupid, and won't output a non-zero error code if there's no main
        // function in the file, so we check if stdout is empty instead
        if !res.stdout.is_empty() {
            bail!("[cmd] {:#?}\n{}", process, fmt_output(&res))
        }

        Ok(res)
    }
}
