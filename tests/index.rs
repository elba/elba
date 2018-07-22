extern crate elba;
extern crate url;

use elba::{index::Index, package::{Name, resolution::DirectRes}, util::lock::DirLock};
use std::{path::PathBuf, str::FromStr};

#[test]
fn index_success() {
    let url = DirectRes::from_str("dir+file://data/index").unwrap();
    let start = env!("CARGO_MANIFEST_DIR");
    let mut path = PathBuf::new();
    path.push(start);
    path.push("tests/data/index");
    let path = DirLock::acquire(&path).unwrap();
    let i = Index::from_disk(url, path).unwrap();

    let vs = i.entries(&Name::from_str("no_conflict/root").unwrap());
    vs.unwrap();
    // assert!(vs.is_ok())
}
