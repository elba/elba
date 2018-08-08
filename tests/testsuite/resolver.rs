use super::util::{index, CACHE};
use elba::{
    index::Indices,
    package::{
        resolution::{DirectRes, IndexRes, Resolution},
        Name, PackageId, Summary,
    },
    resolve::Resolver,
    retrieve::Retriever,
    util::graph::Graph,
};
use semver::Version;
use std::str::FromStr;

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
    let v = vec![index()];

    Indices::new(v)
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
        Graph::default(),
        def_ix,
    )
}

fn resolver<'a>(retriever: &'a mut Retriever<'static>) -> Resolver<'a, 'static> {
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
