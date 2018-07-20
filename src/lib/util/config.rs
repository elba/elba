//! Global `elba` configuration file specification.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.
//!
//! Environment variables (.env files?) should also be able to modify the configuration.

use indexmap::IndexMap;

// TODO: Indices
#[derive(Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_aliases")]
    pub alias: IndexMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            alias: default_aliases(),
        }
    }
}

fn default_aliases() -> IndexMap<String, String> {
    indexmap!(
        "i".to_owned() => "install".to_owned(),
        "b".to_owned() => "build".to_owned(),
        "t".to_owned() => "test".to_owned(),
        "r".to_owned() => "run".to_owned(),
    )
}
