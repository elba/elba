extern crate elba;
#[macro_use]
extern crate lazy_static;
extern crate semver;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;
extern crate url;

use elba::{
    index::{Index, Indices},
    package::{
        resolution::{DirectRes, IndexRes, Resolution},
        Name, PackageId, Summary,
    },
    resolve::{solve::Solve, Resolver},
    retrieve::{Cache, Retriever},
    util::lock::DirLock,
};
use semver::Version;
use slog::*;
use std::{path::PathBuf, str::FromStr};

lazy_static! {
    static ref LOGGER: Logger = new_logger();
    static ref CACHE: Cache = cache();
}

macro_rules! sum {
    ($a:tt, $b:tt) => {{
        let root_name = Name::from_str($a).unwrap();
        let root_pkg = PackageId::new(
            root_name,
            Resolution::Index(IndexRes::from_str("index+dir+data/index/").unwrap()),
        );
        Summary::new(root_pkg, Version::parse($b).unwrap())
    }};
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

// Even though we could use &CACHE.get_indices, we don't here.
//
// Cache assumes that for an Index located on disk, the public-facing DirectRes and the local path
// perfectly match, which is a reasonable real-world assumption. However, we can't do the env! trick
// in the indices themselves, where you HAVE to specify the index of a package's dependencies,
// which would mean that I'd end up having to hard-code my personal directory tree :v
//
// One reasonable solution would be to change index.toml such that index dependencies are specified
// in IndexMap form, and indices would be referred to in dependencies by their "short" name. A method
// dependencies() would turn short names into proper `DirectRes` structs. By doing this, we could
// do the env! trick within just the index.toml file.
fn indices() -> Indices {
    let url = DirectRes::from_str("dir+data/index/").unwrap();
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/index");

    let path = DirLock::acquire(&path).unwrap();

    let v = vec![Index::from_disk(url, path).unwrap()];

    Indices::new(v)
}

fn cache() -> Cache {
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/cache");

    Cache::from_disk(&LOGGER, path)
}

fn retriever(root: Summary) -> Retriever<'static> {
    let mut ixs = indices();

    let root_deps = ixs
        .select(&root)
        .unwrap()
        .dependencies
        .iter()
        .cloned()
        .map(|d| (PackageId::new(d.name, Resolution::Index(d.index)), d.req))
        .collect::<Vec<_>>();

    let def_ix = IndexRes {
        res: DirectRes::from_str("dir+data/index/").unwrap(),
    };

    Retriever::new(
        &CACHE.logger.clone(),
        &CACHE,
        root,
        root_deps,
        ixs,
        Solve::default(),
        def_ix,
    )
}

fn resolver<'a>(retriever: &'a mut Retriever<'a>) -> Resolver<'a> {
    Resolver::new(&retriever.logger.clone(), retriever)
}

#[test]
fn resolve_no_conflict() {
    let mut retriever = retriever(sum!("no_conflict/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_avoid_conflict() {
    let mut retriever = retriever(sum!("avoid_conflict/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_res_simple() {
    let mut retriever = retriever(sum!("conflict_res_simple/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_res_partial() {
    let mut retriever = retriever(sum!("conflict_res_partial/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_simple_report() {
    let mut retriever = retriever(sum!("conflict_simple/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    let msg = resolver.solve();
    assert!(msg.is_err())
}

#[test]
fn resolve_conflict_complex_report() {
    let mut retriever = retriever(sum!("conflict_complex/root", "1.0.0"));
    let resolver = resolver(&mut retriever);
    let msg = resolver.solve();
    assert!(msg.is_err())
}
