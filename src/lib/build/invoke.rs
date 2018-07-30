//! Utilities for interacting with the Idris compiler

use std::{fs, path::Path};

use build::context::BuildContext;
use retrieve::cache::{Binary, Layout};
use util::errors::Res;

// CompileInvocation is responsible for dealing with building libraries (ibc stuff)
#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub pkg: &'a Binary,
    pub deps: &'a [Binary],
    pub targets: &'a [String],
    pub layout: &'a Layout,
}

impl<'a> CompileInvocation<'a> {
    pub fn exec(self, bcx: &BuildContext) -> Res<()> {
        if let Some(s) = self.pkg.source() {
            let lib_target = s.meta.targets.lib.clone().ok_or_else(|| {
                format_err!(
                    "internal: package {} does not contain lib target",
                    s.meta.package.name
                )
            })?;

            // Because we check if the path is inside the package in Manifest::from_str, this is ok.
            let src_path = s.path.path().join(lib_target.path);
            let targets = self
                .targets
                .iter()
                .map(|mod_name| {
                    src_path
                        .join(mod_name.replace(".", "/"))
                        .with_extension("idr")
                })
                .collect::<Vec<_>>();

            self.compile(bcx)?;

            for from in targets {
                let to = self
                    .pkg
                    .target
                    .path()
                    .join(from.strip_prefix(&src_path).unwrap());

                fs::create_dir_all(to.parent().unwrap())?;
                fs::rename(from, to)?;
            }
        }

        Ok(())
    }

    fn compile(&self, bcx: &BuildContext) -> Res<()> {
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
            process.arg(self.layout.build.join(target));
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
    pub build: &'a Path,
    pub deps: &'a [Binary],
    pub output: String,
    pub backend: String,
    pub layout: &'a Layout,
    /// Whether the output should be treated as a binary (true) or artifact files (false)
    pub binary: bool,
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
            .arg(&self.build);

        for binary in self.deps {
            // We assume that the binary has already been compiled
            process.arg("-i").arg(binary.target.path());
        }

        process.spawn()?;

        Ok(())
    }
}
