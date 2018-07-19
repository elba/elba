//! Utilities for interacting with the Idris compiler

use std::fs;
use std::path::PathBuf;

use failure::{err_msg, Error};

use super::context::BuildContext;
use package::Name;
use retrieve::cache::Source;
use util::lock::DirLock;

#[derive(Debug)]
pub struct CompileInvocation {
    src: PathBuf,
    dep_builds: Vec<(Name, PathBuf)>,
    build_dir: BuildDir,
}

impl CompileInvocation {
    pub fn execute(&mut self, bcx: &mut BuildContext) -> Result<(), Error> {
        // Setup file structure of dependencies 
        for (name, dep_build) in &self.dep_builds {
            if dep_build
                .extension()
                .filter(|ext| ext.to_str() == Some("ibc"))
                .is_none()
            {
                bail!("Dependency build '{:?}' is supposed to be .ibc", dep_build);
            }

            let mut dep_file_dest = self.build_dir.build.join(name.group());
            dep_file_dest.set_file_name(name.name());
            dep_file_dest.set_extension("ibc");

            fs::copy(dep_build, dep_file_dest)?;
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

#[derive(Debug)]
pub struct BuildDir {
    lock: DirLock,
    pub root: PathBuf,
    pub build: PathBuf,
}

impl BuildDir {
    pub fn new(lock: DirLock) -> Result<Self, Error> {
        let root = lock.path().to_path_buf();

        let layout = BuildDir {
            lock,
            root: root.clone(),
            build: root.join("build"),
        };

        fs::create_dir(&layout.root)?;
        fs::create_dir(&layout.build)?;

        Ok(layout)
    }
}
