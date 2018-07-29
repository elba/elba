//! Utilities for interacting with the Idris compiler

use std::path::{Path, PathBuf};

use build::{context::BuildContext, Layout};
use retrieve::Binary;
use util::{clear_dir, copy_dir, errors::Res};

#[derive(Debug)]
pub struct CompileInvocation<'a> {
    pub src: &'a Path,
    pub deps: &'a Vec<Binary>,
    // A target is a relative path from source root to a source file.
    // Targets can be modules of lib or main entry of binary. All targets will get compiled.
    pub targets: &'a Vec<PathBuf>,
    pub layout: &'a Layout,
}

impl<'a> CompileInvocation<'a> {
    pub fn execute(self, bcx: &BuildContext) -> Res<()> {
        clear_dir(&self.layout.build)?;

        // setup source
        copy_dir(self.src, &self.layout.build)?;

        // invoke compiler
        for target in self.targets {
            let target = self.layout.build.join(target);

            let mut process = bcx.compiler.process();
            process.current_dir(&self.layout.build).arg("--check");

            // include dependencies
            for binary in self.deps {
                process.arg("-i").arg(binary.target());
            }

            // compile units
            for target in self.targets {
                process.arg(self.layout.build.join(target));
            }

            process.spawn()?;

            let target_bin = target.with_extension("ibc");
            if !target_bin.exists() {
                bail!(
                    "Compiler does not generate binary for {}",
                    target.to_string_lossy()
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct CodegenInvocation {
    build: PathBuf,
    output: String,
    backend: String,
    layout: Layout,
}

impl CodegenInvocation {
    pub fn execute(self, bcx: &BuildContext) -> Res<()> {
        // invoke compiler
        bcx.compiler
            .process()
            .current_dir(&self.layout.bin)
            .args(&["--codegen", &self.backend])
            .args(&["-o", &self.output])
            .arg(&self.build)
            .spawn()?;

        Ok(())
    }
}
