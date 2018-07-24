//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

use failure::{Error, ResultExt};
use indexmap::IndexSet;
use toml;

use super::*;

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct LockfileToml {
    pub packages: IndexSet<LockedPkg>,
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct LockedPkg {
    #[serde(flatten)]
    pub sum: Summary,
    #[serde(default = "Vec::new")]
    pub dependencies: Vec<Summary>,
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
id = "terminator/one@index+tar+https://elba.io/pkg"
version = "0.1.4"

[[packages]]
id = "good/package@dir+/here/there"
version = "1.0.5-alpha.5-zeta.15"
dependencies = [
    { id = "terminator/one@index+tar+https://elba.io/pkg", version = "0.1.4" }
]
        "#;

        assert!(LockfileToml::from_str(lockfile).is_ok());
    }
}
