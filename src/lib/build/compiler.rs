//! Utilities for interacting with the Idris compiler

use std::fs;
use std::path::PathBuf;

use failure::{err_msg, Error};

use super::{BuildContext, TargetDir};

#[derive(Debug)]
pub struct CompileInvocation {
    src: PathBuf,
    deps_build: Vec<PathBuf>,
    target_dir: TargetDir,
}

impl CompileInvocation {
    pub fn excute(&mut self, bcx: &mut BuildContext) -> Result<(), Error> {
        for dep in &self.deps_build {
            if dep
                .extension()
                .filter(|ext| ext.to_str() == Some("ibc"))
                .is_none()
            {
                bail!("Dependency build '{:?}' is supposed to be .ibc", dep);
            }

            // TODO: Error struct
            let dep_file_name = dep
                .file_name()
                .ok_or(err_msg("Dependency build refs to a non-file"))?;
            fs::copy(dep, self.target_dir.build.join(dep_file_name))?;
        }

        // TODO: Error struct
        let src_file_name = &self
            .src
            .file_name()
            .ok_or(err_msg("Src refs to a non-file"))?;
        let src_dest = self.target_dir.build.join(src_file_name);
        fs::copy(&self.src, &src_dest)?;

        let mut target = src_dest.clone();
        target.set_extension("ibc");

        // Invoke compiler
        bcx.compiler
            .process()
            .cwd(&self.target_dir.build)
            .arg("--check")
            .arg(src_file_name)
            .exec()?;

        if !target.exists() {
            bail!(
                "Compilation of {} does not generate binary",
                self.src.to_string_lossy()
            );
        }

        fs::copy(
            &target,
            self.target_dir.root.join(&target.file_name().unwrap()),
        )?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct CodegenInvocation {
    src: PathBuf,
    output: String,
    target_dir: TargetDir,
}

impl CodegenInvocation {
    pub fn excute(&mut self, bcx: &mut BuildContext) -> Result<(), Error> {
        let backend: &str = bcx.cfg.backend.as_ref().unwrap();

        // Invoke compiler
        bcx.compiler
            .process()
            .cwd(&self.target_dir.root)
            .args(&["--codegen", backend])
            .args(&["-o", &self.output])
            .arg(&self.src)
            .exec()?;

        Ok(())
    }
}
