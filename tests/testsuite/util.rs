use elba::{
    remote::{
        resolution::{DirectRes, IndexRes},
        Index,
    },
    retrieve::cache::{Cache, Layout},
    util::{copy_dir, lock::DirLock, shell::Shell},
};
use indexmap::{indexmap, IndexMap};
use lazy_static::lazy_static;
use slog::{self, o, Logger};
use std::{path::PathBuf, str::FromStr};
use tempdir::TempDir;

lazy_static! {
    pub static ref INDEX_DIR: TempDir = index_dir();
    pub static ref CACHE_DIR: TempDir = cache_dir();
    pub static ref LOGGER: Logger = new_logger();
    pub static ref CACHE: Cache = cache();
    pub static ref IXMAP: IndexMap<String, IndexRes> = indexmap!("testing".to_string() => IndexRes {
        res: DirectRes::from_str("dir+data/index/").unwrap(),
    });
}

fn new_logger() -> Logger {
    // Suppress logging output during tests - we don't need to see it
    Logger::root(slog::Discard, o!())
}

pub fn index() -> Index {
    let url = DirectRes::from_str("dir+data/index").unwrap();
    let path = DirLock::acquire(&INDEX_DIR.path()).unwrap();
    Index::from_disk(url, path).unwrap()
}

pub fn shell() -> Shell {
    Shell::default()
}

pub fn cache() -> Cache {
    let layout = Layout {
        bin: CACHE_DIR.path().join("bin"),
        build: CACHE_DIR.path().join("build"),
        indices: CACHE_DIR.path().join("indices"),
        src: CACHE_DIR.path().join("src"),
        tmp: CACHE_DIR.path().join("tmp"),
    };

    Cache::from_disk(&LOGGER, layout, shell()).unwrap()
}

fn index_dir() -> TempDir {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/index");

    let tmp = TempDir::new("elba").unwrap();

    copy_dir(&path, tmp.path(), false).unwrap();

    tmp
}

fn cache_dir() -> TempDir {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/cache");

    let tmp = TempDir::new("elba").unwrap();

    copy_dir(&path, tmp.path(), false).unwrap();

    tmp
}
