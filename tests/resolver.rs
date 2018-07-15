extern crate elba;
extern crate semver;
extern crate url;

use elba::{
    index::{Index, Indices},
    package::{
        lockfile::Lockfile,
        resolution::{IndexRes, Resolution},
        Name, PackageId, Summary,
    },
    resolve::Resolver,
    retrieve::{Cache, Retriever},
    util::DirLock,
};
use semver::Version;
use std::{path::PathBuf, str::FromStr};
use url::Url;

macro_rules! sum {
    ($a:tt, $b:tt) => {{
        let root_name = Name::from_str($a).unwrap();
        let root_pkg = PackageId::new(
            root_name,
            Resolution::Index(IndexRes::from_str("index+file://data/index/").unwrap()),
        );
        Summary::new(root_pkg, Version::parse($b).unwrap())
    }};
}

fn indices() -> Indices {
    let url = Url::from_str("file://data/index/").unwrap();
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/index");

    let path = DirLock::acquire(path);

    let v = vec![Index::from_disk(url, path).unwrap()];

    Indices::new(v)
}

fn cache() -> Cache {
    let ix_url = Url::from_str("file://data/index/").unwrap();
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/cache");

    let def_ix = IndexRes { url: ix_url };
    Cache::from_disk(path, def_ix)
}

fn resolver(root: Summary) -> Resolver {
    let mut ixs = indices();

    let root_deps = ixs
        .select(&root)
        .unwrap()
        .dependencies
        .iter()
        .cloned()
        .map(|d| (PackageId::new(d.name, Resolution::Index(d.index)), d.req))
        .collect::<Vec<_>>();

    let retriever = Retriever::new(cache(), root, root_deps, ixs, Lockfile::default());

    Resolver::new(retriever)
}

#[test]
fn resolve_no_conflict() {
    let mut resolver = resolver(sum!("no_conflict/root", "1.0.0"));

    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_avoid_conflict() {
    let mut resolver = resolver(sum!("avoid_conflict/root", "1.0.0"));

    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_res_simple() {
    let mut resolver = resolver(sum!("conflict_res_simple/root", "1.0.0"));

    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_res_partial() {
    let mut resolver = resolver(sum!("conflict_res_partial/root", "1.0.0"));
    assert!(resolver.solve().is_ok())
}

#[test]
fn resolve_conflict_simple_report() {
    let mut resolver = resolver(sum!("conflict_simple/root", "1.0.0"));
    let msg = resolver.solve();
    println!();
    println!("{}", msg.clone().unwrap_err());
    assert!(msg.is_err())
}

#[test]
fn resolve_conflict_complex_report() {
    let mut resolver = resolver(sum!("conflict_complex/root", "1.0.0"));
    let msg = resolver.solve();
    println!();
    println!("{}", msg.clone().unwrap_err());
    assert!(msg.is_err())
}
