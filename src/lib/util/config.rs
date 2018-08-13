//! Global `elba` configuration file specification.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.
//!
//! Environment variables (.env files?) should also be able to modify the configuration.

use config;
use directories::BaseDirs;
use indexmap::IndexMap;
use package::resolution::DirectRes;
use std::{env, path::PathBuf};
use url::Url;

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub profile: Option<Profile>,
    #[serde(default)]
    pub term: Term,
    #[serde(default = "default_aliases")]
    pub alias: IndexMap<String, String>,
    #[serde(default)]
    pub directories: Directories,
    #[serde(default = "default_indices")]
    pub indices: Vec<DirectRes>,
    #[serde(default)]
    pub backend: Vec<Backend>,
}

impl Config {
    pub fn new() -> Result<Config, config::ConfigError> {
        let mut c = config::Config::new();

        let cwd = env::current_dir();

        c.merge(
            config::File::with_name(
                format!(
                    "{}/.elba/config",
                    BaseDirs::new().unwrap().home_dir().display()
                ).as_ref(),
            ).format(config::FileFormat::Toml)
            .required(false),
        )?;

        if let Ok(cwd) = cwd {
            for parent in cwd.ancestors().collect::<Vec<_>>().iter().rev() {
                c.merge(
                    config::File::from(parent.join(".elba/config"))
                        .format(config::FileFormat::Toml)
                        .required(false),
                )?;
            }
        }

        c.merge(config::Environment::with_prefix("elba"))?;

        c.try_into()
    }

    pub fn verbosity(&mut self, v: Verbosity) -> &mut Config {
        self.term.verbosity = v;
        self
    }

    pub fn color(&mut self, c: bool) -> &mut Config {
        self.term.color = c;
        self
    }

    pub fn default_backend(&self) -> Backend {
        self.backend
            .iter()
            .find(|x| x.default)
            .cloned()
            .unwrap_or_else(Backend::default)
    }

    pub fn get_backend(&self, name: &str) -> Option<Backend> {
        self.backend.iter().find(|x| x.name == name).cloned()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            profile: None,
            term: Term::default(),
            alias: default_aliases(),
            directories: Directories::default(),
            indices: Vec::default(),
            backend: Vec::new(),
        }
    }
}

fn default_aliases() -> IndexMap<String, String> {
    indexmap!(
        "i".to_owned() => "install".to_owned(),
        "b".to_owned() => "build".to_owned(),
        "t".to_owned() => "test".to_owned(),
    )
}

fn default_indices() -> Vec<DirectRes> {
    let repo = Url::parse("https://github.com/elba/index").unwrap();
    let tag = "master".to_string();
    vec![DirectRes::Git { repo, tag }]
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Directories {
    pub cache: PathBuf,
}

impl Default for Directories {
    fn default() -> Self {
        Directories {
            // TOOD: no unwrapperino pls
            cache: BaseDirs::new().unwrap().home_dir().join(".elba"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Backend {
    pub name: String,
    pub default: bool,
    pub portable: bool,
    pub runner: Option<String>,
    pub opts: Vec<String>,
    pub extension: Option<String>,
}

impl Default for Backend {
    fn default() -> Self {
        Backend {
            name: "c".to_string(),
            default: false,
            portable: false,
            runner: None,
            opts: Vec::default(),
            extension: None,
        }
    }
}
