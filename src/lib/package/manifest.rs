//! `manifest.rs` - Parsing package manifests: Pkg.toml

use failure::ResultExt;
use std::{collections::BTreeMap, str::FromStr};
use toml;

use err::*;

/// A relative file path (not module path)
type PathV = String;

#[derive(Deserialize, Debug, Serialize)]
pub struct Spec(Name, Version);

#[derive(Deserialize, Debug, Serialize)]
pub struct Name(String);

#[derive(Deserialize, Debug, Serialize)]
pub struct Version(String);

fn default_empty_vec<T>() -> Vec<T> {
    vec![]
}

fn default_empty_map<K: Ord, V>() -> BTreeMap<K, V> {
    BTreeMap::new()
}

#[derive(Deserialize, Debug)]
struct Manifest {
    package: Package,
    #[serde(default = "default_empty_map")]
    dependencies: BTreeMap<Name, DepSpec>,
    #[serde(default = "default_empty_map")]
    dev_dependencies: BTreeMap<Name, DepSpec>,
    targets: Targets,
    #[serde(default)]
    features: Features,
}

impl FromStr for Manifest {
    type Err = Error; // TODO

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw).context(ErrorKind::InvalidManifest).map_err(Error::from)
    }
}

#[derive(Deserialize, Debug)]
struct Package {
    name: Name,
    version: Version,
    authors: Vec<String>,
    license: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum DepSpec {
    RegSpec(Version),
    Registry {
        version: Version,
        #[serde(default = "default_empty_vec")]
        features: Vec<String>,
    },
    Local {
        path: String,
        #[serde(default = "default_empty_vec")]
        features: Vec<String>,
    },
    Git {
        git: String,
        #[serde(default = "default_dep_spec_branch")]
        branch: String,
        #[serde(flatten)]
        spec: Option<PkgGitSpecifier>,
        #[serde(default = "default_empty_vec")]
        features: Vec<String>,
    },
}

fn default_dep_spec_branch() -> String {
    "master".to_owned()
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum PkgGitSpecifier {
    #[serde(rename = "commit")]
    Commit(String),
    #[serde(rename = "tag")]
    Tag(String),
}

#[derive(Deserialize, Debug)]
struct Targets {
    lib: Option<LibTarget>,
    #[serde(default = "default_empty_vec")]
    bin: Vec<Target>,
    #[serde(default = "default_empty_vec")]
    test: Vec<Target>,
    #[serde(default = "default_empty_vec")]
    bench: Vec<Target>,
}

#[derive(Deserialize, Debug)]
struct Target {
    name: String,
    path: PathV, // TODO: paths?
}

#[derive(Deserialize, Debug)]
struct LibTarget {
    name: String,
    exports: Vec<PathV>,
}

#[derive(Deserialize, Debug)]
struct Features {
    default: Vec<String>,
    #[serde(flatten)]
    other: BTreeMap<String, Vec<String>>,
}

impl Default for Features {
    fn default() -> Self {
        Features {
            default: vec![],
            other: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_manifest() {
        let manifest = "
[package]
name = 'cool/beans'
version = '1.0.0'
authors = ['me']
license = 'MIT'

[dependencies]
'awesome/a' = '1.0.0'
'cool/b' = { git = 'https://github.com/super/cool', branch = 'this_one', tag = '1.0.0' }
'great/c' = { path = 'here/right/now' }

[dev_dependencies]
'ayy/x' = '2.0'

[[targets.bin]]
name = 'bin1'
path = 'src/bin/Here.idr'

[targets.lib]
name = 'lib1'
exports = [
    'src/lib/This.idr',
    'src/lib/That.idr',
]

[features]
default = ['all']
all = []
        ";

        println!("{:#?}", Manifest::from_str(manifest));

        assert!(Manifest::from_str(manifest).is_ok());
    }
}
