//! Actually building Idris packages.

pub mod compile;

pub mod context;
pub mod job;

use std::fs;
use std::path::PathBuf;
use util::{errors::Res, lock::DirLock};

#[derive(Debug)]
pub struct Layout {
    lock: DirLock,
    pub root: PathBuf,
    pub bin: PathBuf,
    pub lib: PathBuf,
    pub build: PathBuf,
    pub deps: PathBuf,
}

impl Layout {
    pub fn new(lock: DirLock) -> Res<Self> {
        let root = lock.path().to_path_buf();

        let layout = Layout {
            lock,
            root: root.clone(),
            bin: root.join("bin"),
            lib: root.join("lib"),
            build: root.join("build"),
            deps: root.join("deps"),
        };

        fs::create_dir(&layout.root)?;
        fs::create_dir(&layout.bin)?;
        fs::create_dir(&layout.lib)?;
        fs::create_dir(&layout.build)?;
        fs::create_dir(&layout.deps)?;

        Ok(layout)
    }
}
