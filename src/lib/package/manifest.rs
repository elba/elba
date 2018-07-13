//! Package manifest files.

use self::version::Constraint;
use super::{resolution::IndexRes, *};
use err::*;
use failure::ResultExt;
use indexmap::IndexMap;
use semver::Version;
use std::str::FromStr;
use toml;
use url::Url;
use url_serde;

/// A relative file path (not module path)
type PathV = String;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    package: PackageInfo,
    #[serde(default = "IndexMap::new")]
    pub dependencies: IndexMap<Name, DepReq>,
    #[serde(default = "IndexMap::new")]
    dev_dependencies: IndexMap<Name, DepReq>,
    targets: Targets,
}

impl FromStr for Manifest {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw)
            .context(ErrorKind::InvalidManifestFile)
            .map_err(Error::from)
    }
}

#[derive(Deserialize, Debug)]
struct PackageInfo {
    name: Name,
    version: Version,
    authors: Vec<String>,
    license: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum DepReq {
    Registry(Constraint),
    RegLong {
        con: Constraint,
        registry: IndexRes,
    },
    Local {
        #[serde(with = "url_serde")]
        path: Url,
    },
    Git {
        #[serde(with = "url_serde")]
        git: Url,
        #[serde(default)]
        #[serde(flatten)]
        spec: PkgGitSpecifier,
        #[serde(default = "String::new")]
        sub_path: String,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PkgGitSpecifier {
    Branch(String),
    Commit(String),
    Tag(String),
}

impl Default for PkgGitSpecifier {
    fn default() -> Self {
        PkgGitSpecifier::Branch("master".to_string())
    }
}

#[derive(Deserialize, Debug)]
struct Targets {
    lib: Option<LibTarget>,
    #[serde(default = "Vec::new")]
    bin: Vec<Target>,
    #[serde(default = "Vec::new")]
    test: Vec<Target>,
    #[serde(default = "Vec::new")]
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
    other: IndexMap<String, Vec<String>>,
}

impl Default for Features {
    fn default() -> Self {
        Features {
            default: vec![],
            other: IndexMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_manifest() {
        let manifest = r#"
[package]
name = 'ring_ding/test'
version = '1.0.0'
authors = ['me']
license = 'MIT'

[dependencies]
'awesome/a' = '>= 1.0.0 < 2.0.0'
'cool/b' = { git = 'https://github.com/super/cool', tag = "v1.0.0" }
'great/c' = { path = 'file://here/right/now' }

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
]"#;

        assert!(Manifest::from_str(manifest).is_ok());
    }
}
