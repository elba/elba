//! `lockfile.rs` - logic for creating and reading Lockfiles.
//!
//! Lockfiles are created based on dependency constraints, and ensure that builds are repeatable

use spec::*;

#[derive(Deserialize, Debug, Serialize)]
struct Lockfile {
    packages: Vec<LockedPkg>,
}

#[derive(Deserialize, Debug, Serialize)]
struct LockedPkg {
    name: String,
    version: Spec,
    // TODO: Other things
}
