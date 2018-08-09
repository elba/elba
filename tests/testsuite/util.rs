use elba::{
    index::Index,
    package::resolution::DirectRes,
    retrieve::Cache,
    util::{copy_dir, lock::DirLock},
};
use slog::{self, Logger};
use std::{path::PathBuf, str::FromStr};
use tempdir::TempDir;

lazy_static! {
    pub static ref INDEX_DIR: TempDir = index_dir();
    pub static ref CACHE_DIR: TempDir = cache_dir();
    pub static ref LOGGER: Logger = new_logger();
    pub static ref CACHE: Cache = cache();
}

fn new_logger() -> Logger {
    /*
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    */

    // Suppress logging output during tests - we don't need to see it
    Logger::root(slog::Discard, o!())
}

pub fn index() -> Index {
    let url = DirectRes::from_str("dir+data/index").unwrap();
    let path = DirLock::acquire(&INDEX_DIR.path()).unwrap();
    Index::from_disk(url, path).unwrap()
}

pub fn cache() -> Cache {
    Cache::from_disk(&LOGGER, &CACHE_DIR.path()).unwrap()
}

fn index_dir() -> TempDir {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/index");

    let tmp = TempDir::new("elba").unwrap();

    copy_dir(&path, tmp.path()).unwrap();

    tmp
}

fn cache_dir() -> TempDir {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/cache");

    let tmp = TempDir::new("elba").unwrap();

    copy_dir(&path, tmp.path()).unwrap();

    tmp
}
