//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

use std::collections::HashSet;
use toml;

use super::*;

#[derive(Deserialize, Debug, Serialize)]
pub struct Lockfile {
    package: HashSet<LockedPkg>,
}

pub type LockedPkg = Summary<PackageId>;

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
id = "terminator/one 0.2.4 dir+file:///right/here"
checksum = { fmt = "sha512", checksum = "334d016f755cd6dc58c53a86e183882f8ec14f52fb05345887c8a5edd42c87b7" }

[[package]]
id = "good/package 1.0.5-alpha.5-zeta.15 local+file:///right/there"
dependencies = [
    "better/package 92.4.24 index+https://matic.io/pkg"
]
checksum = { fmt = "sha512", checksum = "4a7d6d3e8888a86b41c710f1d44c43d9ec7a4f97dce4f1ec3c0fb124ca0188de" }
        "#;

        println!("{:#?}", Lockfile::from_str(lockfile));

        assert!(Lockfile::from_str(lockfile).is_ok());
    }
}
