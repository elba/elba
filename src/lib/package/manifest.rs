//! Package manifest files.

use self::version::Constraint;
use super::{
    resolution::{DirectRes, IndexRes},
    *,
};
use failure::ResultExt;
use indexmap::IndexMap;
use semver::Version;
use std::{path::PathBuf, str::FromStr};
use toml;
use url::Url;
use url_serde;
use util::err::*;

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
    #[serde(default)]
    workspace: IndexMap<Name, String>,
}

impl Manifest {
    pub fn version(&self) -> &Version {
        &self.package.version
    }
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
        path: PathBuf,
    },
    Git {
        #[serde(with = "url_serde")]
        git: Url,
        #[serde(default)]
        #[serde(flatten)]
        spec: PkgGitSpecifier,
    },
}

impl DepReq {
    pub fn into_dep(self, def_index: IndexRes, n: Name) -> (PackageId, Constraint) {
        match self {
            DepReq::Registry(c) => {
                let pi = PackageId::new(n, def_index.into());
                (pi, c)
            }
            DepReq::RegLong { con, registry } => {
                let pi = PackageId::new(n, registry.into());
                (pi, con)
            }
            DepReq::Local { path } => {
                let res = DirectRes::Dir { url: path };
                let pi = PackageId::new(n, res.into());
                (pi, Constraint::any())
            }
            DepReq::Git { git, spec } => {
                let res = DirectRes::Git {
                    repo: git,
                    tag: spec,
                };
                let pi = PackageId::new(n, res.into());
                (pi, Constraint::any())
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PkgGitSpecifier {
    Branch(String),
    Commit(String),
    Tag(String),
}

impl FromStr for PkgGitSpecifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.splitn(2, '=');
        let fmt = s.next().unwrap();
        let spec = s
            .next()
            .ok_or_else(|| ErrorKind::InvalidSourceUrl)?
            .to_string();

        match fmt {
            "branch" => Ok(PkgGitSpecifier::Branch(spec)),
            "commit" => Ok(PkgGitSpecifier::Commit(spec)),
            "tag" => Ok(PkgGitSpecifier::Tag(spec)),
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for PkgGitSpecifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PkgGitSpecifier::Branch(a) => write!(f, "branch={}", a),
            PkgGitSpecifier::Commit(a) => write!(f, "branch={}", a),
            PkgGitSpecifier::Tag(a) => write!(f, "branch={}", a),
        }
    }
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
