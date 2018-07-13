//! Global `elba` configuration file specification.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.

// TODO: A Config file. Should deal with registries, etc. Should be unified.
#[derive(Deserialize, Serialize)]
struct Config {}
