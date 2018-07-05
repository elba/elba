extern crate matic;
extern crate semver;
extern crate url;

use matic::{
    index::{Index, Indices},
    package::{lockfile::Lockfile, IndexRes, Name, PackageId, Resolution, Summary},
    resolve::Resolver,
};
use semver::Version;
use std::{path::PathBuf, str::FromStr};
use url::Url;

macro_rules! sum {
    ($a:tt, $b:tt) => {{
        let root_name = Name::from_str($a).unwrap();
        let root_pkg = PackageId::new(
            root_name,
            Resolution::Index(IndexRes::from_str("index+file://index/").unwrap()),
        );
        Summary::new(root_pkg, Version::parse($b).unwrap())
    }};
}

fn indices() -> Indices {
    let url = Url::from_str("file://index").unwrap();
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/index");

    let v = vec![Index::from_local(url, path).unwrap()];

    Indices::new(v)
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

    Resolver::new(root, root_deps, ixs, Lockfile::default())
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
