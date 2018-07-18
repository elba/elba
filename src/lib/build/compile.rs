//! Utilities for interacting with the Idris compiler

use std::fs;
use std::path::PathBuf;

use failure::{err_msg, Error};

use super::context::{BuildConfig, BuildContext, BuildDir};

#[derive(Debug)]
pub struct CompileInvocation {
    src: PathBuf,
    deps_build: Vec<PathBuf>,
    build_dir: BuildDir,
}

impl CompileInvocation {
    pub fn execute(&mut self, bcx: &mut BuildContext) -> Result<(), Error> {
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
            fs::copy(dep, self.build_dir.build.join(dep_file_name))?;
        }

        // TODO: Error struct
        let src_file_name = &self
            .src
            .file_name()
            .ok_or(err_msg("Src refs to a non-file"))?;
        let src_dest = self.build_dir.build.join(src_file_name);
        fs::copy(&self.src, &src_dest)?;

        let mut target = src_dest.clone();
        target.set_extension("ibc");

        // Invoke compiler
        bcx.compiler
            .process()
            .cwd(&self.build_dir.build)
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
            self.build_dir.root.join(&target.file_name().unwrap()),
        )?;

        fs::remove_dir_all(&self.build_dir.build)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct CodegenInvocation {
    src: PathBuf,
    output: String,
    backend: String,
    build_dir: BuildDir,
}

impl CodegenInvocation {
    pub fn execute(&mut self, bcx: &mut BuildContext) -> Result<(), Error> {
        // Invoke compiler
        bcx.compiler
            .process()
            .cwd(&self.build_dir.root)
            .args(&["--codegen", &self.backend])
            .args(&["-o", &self.output])
            .arg(&self.src)
            .exec()?;

        Ok(())
    }
}
