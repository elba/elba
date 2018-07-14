//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

// TODO: Should the lockfile store checksums? If so, of what? The tarball? The folder?
// see https://github.com/rust-lang/cargo/issues/4800 for why we'd want this

use failure::ResultExt;
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

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile {
            packages: indexmap!(),
        }
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
    type Err = Error;

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
id = "terminator/one@index+https://elba.io/pkg"
version = "0.1.4"

[[packages]]
id = "good/package@dir+file:///right/there"
version = "1.0.5-alpha.5-zeta.15"
dependencies = [
    { id = "terminator/one@index+https://elba.io/pkg", version = "0.1.4" }
]
        "#;

        println!("{:#?}", Lockfile::from_str(lockfile));

        assert!(Lockfile::from_str(lockfile).is_ok());
    }
}
