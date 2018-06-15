//! `manifest.rs` - Parsing package manifests: Pkg.toml

use std::{collections::BTreeMap, str::FromStr};
use toml;

use spec::*;

type PathV = String;

#[derive(Clone, Debug)]
struct Package {
    // TODO
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PkgVer {
    RegSpec(Spec),
    Registry { version: Spec, /* TODO More here */ },
    Git { url: String, commit: String }
}

#[derive(Deserialize)]
struct Manifest {
    meta: Meta,
    dependencies: BTreeMap<String, String>,
    dev_dependencies: BTreeMap<String, String>,
    // TODO targets
    // TODO features
}

#[derive(Deserialize)]
struct Target {
    name: String,
    path: PathV, // TODO: paths?
}

#[derive(Deserialize)]
struct Meta {
    name: String,
    // TODO: Version parsing
    version: String,
    authors: Vec<String>,
    license: String,
}

impl FromStr for Manifest {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str(s)
    }
}
