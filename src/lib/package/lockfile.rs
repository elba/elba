//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

use indexmap::{IndexMap, IndexSet};
use toml;

use super::*;

#[derive(Clone, Debug)]
pub struct Lockfile {
    pub packages: IndexMap<PackageId, (Version, Vec<Summary>)>,
}

impl FromStr for Lockfile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let toml = LockfileToml::from_str(s)?;

        Ok(toml.into())
    }
}

impl From<LockfileToml> for Lockfile {
    fn from(l: LockfileToml) -> Self {
        let mut packages = indexmap!();
        for package in l.packages {
            let s = package.sum;
            packages.insert(s.id, (s.version, package.dependencies));
        }

        Lockfile { packages }
    }
}

#[derive(Clone, Deserialize, Debug, Serialize)]
struct LockfileToml {
    packages: IndexSet<LockedPkg>,
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, Eq, Hash)]
struct LockedPkg {
    #[serde(flatten)]
    sum: Summary,
    #[serde(default = "Vec::new")]
    dependencies: Vec<Summary>,
}

impl FromStr for LockfileToml {
    type Err = Error; // TODO

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw)
            .context(ErrorKind::InvalidLockfile)
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lockfile() {
        let lockfile = r#"
[[packages]]
id = "terminator/one index+https://matic.io/pkg"
version = "0.1.4"
checksum = { fmt = "sha512", hash = "334d016f755cd6dc58c53a86e183882f8ec14f52fb05345887c8a5edd42c87b7" }

[[packages]]
id = "good/package dir+file:///right/there"
version = "1.0.5-alpha.5-zeta.15"
dependencies = [
    { id = "terminator/one index+https://matic.io/pkg", version = "0.1.4" }
]
checksum = { fmt = "sha512", hash = "4a7d6d3e8888a86b41c710f1d44c43d9ec7a4f97dce4f1ec3c0fb124ca0188de" }
        "#;

        println!("{:#?}", Lockfile::from_str(lockfile));

        assert!(Lockfile::from_str(lockfile).is_ok());
    }
}
