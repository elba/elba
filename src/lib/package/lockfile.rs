//! Module `package/lockfile` contains logic for (de)serializing lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

// TODO: The representation of a package in a lockfile might end up being the canonical rep we use
//       in code...

use semver::Version;

use super::*;

#[derive(Deserialize, Debug, Serialize)]
struct Lockfile {
    packages: Vec<LockedPkg>,
}

#[derive(Deserialize, Debug, Serialize)]
struct LockedPkg {
    name: Name,
    version: Version,
    // TODO: checksum, deps, src?
}
