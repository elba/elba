//! Utilities for interacting with the Idris compiler

use std::path::PathBuf;

use super::context::BuildContext;
use super::Layout;
use package::Name;
use retrieve::{Binary, Source};
use util::{self, errors::Res};

#[derive(Debug)]
pub struct CompileInvocation {
    pub src: Source,
    pub deps: Vec<(Name, Binary)>,
    pub layout: Layout,
    // A target is a relative path from source root to a source file.
    // Targets can be modules of lib or main entry of binary. All targets will get compiled.
    pub targets: Vec<PathBuf>,
}

impl CompileInvocation {
    pub fn execute(&mut self, bcx: &mut BuildContext) -> Res<Vec<PathBuf>> {
        util::clear_dir(&self.layout.build)?;

        // setup dependencies
        for (name, binary) in &self.deps {
            let dep_dir = self.layout.build.join(name.group()).join(name.name());
            util::copy_dir(binary.target.path(), &dep_dir)?;
        }

        // setup source
        util::copy_dir(self.src.path.path(), &self.layout.build)?;

        let mut target_bins = Vec::new();

        // invoke compiler
        for target in &self.targets {
            let target = self.layout.build.join(target);

            bcx.compiler
                .process()
                .current_dir(&self.layout.build)
                .arg("--check")
                .arg(&target)
                .spawn()?;

            let mut target_bin = target.clone();
            target_bin.set_extension("ibc");

            if target_bin.exists() {
                target_bins.push(target_bin);
            } else {
                bail!(
                    "Compiler does not generate binary for {}",
                    target.to_string_lossy()
                );
            }
        }

        Ok(target_bins)
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
    pub fn execute(&mut self, bcx: &mut BuildContext) -> Res<()> {
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
