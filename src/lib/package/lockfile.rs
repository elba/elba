//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

use std::collections::HashSet;
use toml;

use super::*;

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct Lockfile {
    package: HashSet<LockedPkg>,
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct LockedPkg {
    #[serde(flatten)]
    sum: Summary,
    #[serde(default = "Vec::new")]
    dependencies: Vec<Summary>,
}

impl FromStr for Lockfile {
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
[[package]]
id = "terminator/one dir+file:///right/here"
version = "0.1.4"
checksum = { fmt = "sha512", hash = "334d016f755cd6dc58c53a86e183882f8ec14f52fb05345887c8a5edd42c87b7" }

[[package]]
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
