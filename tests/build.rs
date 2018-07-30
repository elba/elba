extern crate elba;
extern crate petgraph;

use elba::{
    build::{CompileMode, job::{Job, JobQueue}},
    package::{
        resolution::{DirectRes, Resolution},
        Name, PackageId,
    },
    retrieve::cache::{Binary, Source},
    util::{graph::Graph, lock::DirLock},
};
use std::{path::PathBuf, str::FromStr};

macro_rules! pkg {
    ($a:tt) => {{
        let root_name = Name::from_str($a).unwrap();
        PackageId::new(root_name, Resolution::Root)
    }};
}

#[test]
fn job_queue_empty() {
    JobQueue::default().exec().unwrap();
}

#[test]
fn job_queue_single() {
    let start = env!("CARGO_MANIFEST_DIR").to_owned();
    let p = PathBuf::from(start.clone()).join("tests/data/pkgs/one");
    let q = DirLock::acquire(&PathBuf::from(start).join("tests/data/whatever")).unwrap();
    let dir = DirLock::acquire(&p).unwrap();
    let res = DirectRes::Dir {
        url: p.to_path_buf(),
    };

    let j = Job {
        source: Binary::new(Source::from_folder(&pkg!("one/one"), dir, res).unwrap(), q),
        compile_mode: CompileMode::Bin,
    };

    let mut graph = Graph::default();
    graph.inner.add_node(j);

    let jq = JobQueue {
        graph,
    };

    jq.exec().unwrap();
}
