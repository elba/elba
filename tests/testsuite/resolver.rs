use super::util::{shell, CACHE, INDEX_DIR, IXMAP};
use elba::{
    package::{Name, PackageId, Summary},
    remote::{
        resolution::{DirectRes, IndexRes, Resolution},
        Indices,
    },
    resolve::Resolver,
    retrieve::Retriever,
    util::graph::Graph,
};
use itertools::Either::Right;
use semver::Version;
use std::str::FromStr;

macro_rules! sum {
    ($a:tt, $b:tt) => {{
        let index = DirectRes::Dir {
            path: INDEX_DIR.path().to_owned(),
        };
        let root_name = Name::from_str($a).unwrap();
        let root_pkg = PackageId::new(root_name, Resolution::Index(IndexRes { res: index }));
        Summary::new(root_pkg, Version::parse($b).unwrap())
    }};
}

fn indices() -> Indices {
    let index = DirectRes::Dir {
        path: INDEX_DIR.path().to_owned(),
    };

    CACHE.get_indices(&[index], false, false)
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

    Retriever::new(
        &CACHE.logger.clone(),
        &CACHE,
        root,
        root_deps,
        Right(ixs),
        Graph::default(),
        &IXMAP,
        shell(),
        false,
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
