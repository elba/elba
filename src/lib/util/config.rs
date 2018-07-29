//! Global `elba` configuration file specification.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.
//!
//! Environment variables (.env files?) should also be able to modify the configuration.

use directories::BaseDirs;
use indexmap::IndexMap;
use package::resolution::DirectRes;
use std::path::PathBuf;

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub profile: Option<Profile>,
    #[serde(default)]
    pub term: Term,
    #[serde(default = "default_aliases")]
    pub alias: IndexMap<String, String>,
    #[serde(default)]
    pub directories: Directories,
    // First index = default.
    // In future, default for "indices" should be only official index..
    #[serde(default)]
    pub indices: Vec<DirectRes>,
}

impl Config {
    pub fn merge_files(&mut self) -> &mut Config {
        self
    }

    pub fn merge_env(&mut self) -> &mut Config {
        self
    }

    pub fn verbosity(&mut self, v: Verbosity) -> &mut Config {
        self.term.verbosity = v;
        self
    }

    pub fn color(&mut self, c: bool) -> &mut Config {
        self.term.color = c;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            profile: None,
            term: Term::default(),
            alias: default_aliases(),
            directories: Directories::default(),
            indices: Vec::new(),
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

#[derive(Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize, Serialize)]
pub struct Term {
    pub color: bool,
    pub verbosity: Verbosity,
}

impl Default for Term {
    fn default() -> Self {
        Term {
            color: true,
            verbosity: Verbosity::Normal,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Directories {
    pub cache: PathBuf,
    pub rest: PathBuf,
}

impl Default for Directories {
    fn default() -> Self {
        Directories {
            // TOOD: no unwrapperino pls
            cache: BaseDirs::new().unwrap().home_dir().join(".elba"),
            rest: BaseDirs::new().unwrap().home_dir().join(".elba"),
        }
    }
}
