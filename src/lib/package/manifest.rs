//! Module `package/manifest` deals with package manifest files.

use self::version::Constraint;
use super::*;
use err::*;
use failure::ResultExt;
use semver::Version;
use std::{collections::BTreeMap, str::FromStr};
use toml;

/// A relative file path (not module path)
type PathV = String;

#[derive(Deserialize, Debug)]
struct Manifest {
    package: Package,
    #[serde(default = "BTreeMap::new")]
    dependencies: BTreeMap<Name, DepReq>,
    #[serde(default = "BTreeMap::new")]
    dev_dependencies: BTreeMap<Name, DepReq>,
    targets: Targets,
}

impl FromStr for Manifest {
    type Err = Error; // TODO

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw)
            .context(ErrorKind::InvalidManifestFile)
            .map_err(Error::from)
    }
}

#[derive(Deserialize, Debug)]
struct Package {
    name: Name,
    version: Version,
    authors: Vec<String>,
    license: Option<String>,
}

// TODO: Url type
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum DepReq {
    Registry(Constraint),
    RegLong {
        con: Constraint,
        registry: String,
    },
    Local {
        path: String,
    },
    Git {
        git: String,
        #[serde(default)]
        #[serde(flatten)]
        spec: PkgGitSpecifier,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum PkgGitSpecifier {
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
        let manifest = r#"
[package]
name = 'ring_ding/test'
version = '1.0.0'
authors = ['me']
license = 'MIT'

[dependencies]
'awesome/a' = '>= 1.0.0 < 2.0.0'
'cool/b' = { git = 'https://github.com/super/cool', tag = "v1.0.0" }
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
]"#;

        println!("{:#?}", Manifest::from_str(manifest));

        assert!(Manifest::from_str(manifest).is_ok());
    }
}
