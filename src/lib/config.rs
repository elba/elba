//! Module `config` deals with the spec for a configuration file which changes behavior.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.

// TODO: A Config file. Should deal with registries, etc.
#[derive(Deserialize, Serialize)]
struct Config {}

// TODO: Config file unification
