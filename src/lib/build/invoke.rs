//! Utilities for interacting with the Idris compiler

use crate::{
    build::context::{BuildContext, CompilerFlavor},
    retrieve::cache::{Binary, OutputLayout},
    util::{errors::Res, fmt_output},
};
use failure::bail;
use itertools::Itertools;
use std::{
    path::{Path, PathBuf},
    process::Output,
};

// CompileInvocation is responsible for dealing with ibc stuff
#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub deps: &'a [&'a Binary],
    pub targets: &'a [PathBuf],
    pub build: &'a Path,
    pub args: &'a [String],
}

impl<'a> CompileInvocation<'a> {
    pub fn exec(&self, bcx: &BuildContext) -> Res<Output> {
        // invoke compiler
        let mut process = bcx.compiler.process();
        let flavor = bcx.compiler.flavor();
        process.current_dir(&self.build).arg("--check");

        // Include dependencies
        for binary in self.deps {
            if flavor.is_idris2() {
                bail!("The Idris 2 compiler currently doesn't support custom import paths")
            }
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
        let mut process = bcx.compiler.process();
        let flavor = bcx.compiler.flavor();
        let cwd;

        if self.is_artifact {
            if flavor.is_idris1() {
                process.arg("--interface");
            } else {
                bail!("Only the Idris 1 compiler supports library artifacts")
            }
        }

        process
            .current_dir(if self.is_artifact {
                cwd = self.layout.artifacts.join(&bcx.backend.name);
                &cwd
            } else {
                &self.layout.bin
            })
            .args(&["-o", &self.output])
            .args(&[
                if bcx.backend.portable && flavor.is_idris1() {
                    "--portable-codegen"
                } else {
                    "--codegen"
                },
                &bcx.backend.name,
            ]);

        if !bcx.backend.opts.is_empty() {
            if flavor.is_idris1() {
                process
                    .arg("--cg-opt")
                    .arg(bcx.backend.opts.iter().join(" "));
            }
        }

        process.args(self.args);

        for bin in self.binary {
            if flavor.is_idris2() {
                bail!("The Idris 2 compiler currently doesn't support custom import paths")
            }
            process.arg("-i");
            process.arg(bin.parent().unwrap());
            process.arg(bin);
        }

        let res = process.output()?;
        // The Idris compiler is stupid, and won't output a non-zero error code if there's no main
        // function in the file, so we manually check if stdout contains a "main not found" error.
        let stdout = String::from_utf8_lossy(&res.stdout);

        if stdout.contains("No such variable Main.main") || !res.status.success() {
            bail!("[cmd] {:#?}\n{}", process, fmt_output(&res))
        }

        Ok(res)
    }
}
