//! Global `elba` configuration file specification.
//!
//! Configuration files change the behavior of the package manager, and can be both global and local
//! to a package. Whereas package files are about specifying a package, config files specify the
//! behavior of the tool itself.
//!
//! Environment variables (.env files?) should also be able to modify the configuration.

use super::shell::{Shell, Verbosity};
use crate::{
    remote::resolution::{DirectRes, IndexRes},
    retrieve::cache::Layout,
};
use config;
use directories::{BaseDirs, ProjectDirs};
use indexmap::{indexmap, IndexMap};
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};
use url::Url;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_compiler")]
    pub compiler: String,
    #[serde(default)]
    pub profile: Option<Profile>,
    #[serde(default)]
    pub term: Term,
    #[serde(default = "default_aliases")]
    pub alias: IndexMap<String, String>,
    #[serde(default)]
    pub directories: Directories,
    // The purpose of the mapping is to allow for index aliases.
    #[serde(default = "default_indices")]
    pub indices: IndexMap<String, IndexRes>,
    #[serde(default)]
    pub backend: Vec<Backend>,
}

fn default_compiler() -> String {
    "idris".to_string()
}

impl Config {
    pub fn new() -> Result<Config, config::ConfigError> {
        let mut c = config::Config::new();

        let cwd = env::current_dir();

        c.merge(
            config::File::from(BaseDirs::new().unwrap().home_dir().join(".elba/config"))
                .format(config::FileFormat::Toml)
                .required(false),
        )?;

        c.merge(
            config::File::from(
                ProjectDirs::from("", "", "elba")
                    .unwrap()
                    .config_dir()
                    .join("config"),
            )
            .format(config::FileFormat::Toml)
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

    pub fn shell(&self) -> Shell {
        Shell {
            verbosity: self.term.verbosity,
        }
    }

    pub fn layout(&self) -> Layout {
        Layout {
            bin: self.directories.bin.to_path_buf(),
            build: self.directories.cache.join("build"),
            indices: self.directories.cache.join("indices"),
            src: self.directories.cache.join("src"),
            tmp: self.directories.cache.join("tmp"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            compiler: default_compiler(),
            profile: None,
            term: Term::default(),
            alias: default_aliases(),
            directories: Directories::default(),
            indices: IndexMap::default(),
            backend: Vec::default(),
        }
    }
}

fn default_aliases() -> IndexMap<String, String> {
    indexmap!(
        "i".to_owned() => "install".to_owned(),
        "b".to_owned() => "build".to_owned(),
        "c".to_owned() => "check".to_owned(),
        "t".to_owned() => "test".to_owned(),
    )
}

fn default_indices() -> IndexMap<String, IndexRes> {
    let repo = Url::parse("https://github.com/elba/index").unwrap();
    let tag = "master".to_string();
    indexmap!("official".to_string() => DirectRes::Git { repo, tag }.into())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Term {
    #[serde(default = "default_color")]
    pub color: bool,
    #[serde(default)]
    pub verbosity: Verbosity,
}

fn default_color() -> bool {
    true
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
    #[serde(default = "default_bin_dir")]
    pub bin: PathBuf,
    #[serde(default = "default_data_dir")]
    pub data: PathBuf,
    #[serde(default = "default_cache_dir")]
    pub cache: PathBuf,
}

fn default_bin_dir() -> PathBuf {
    BaseDirs::new().unwrap().home_dir().join(".elba/bin")
}

fn default_data_dir() -> PathBuf {
    ProjectDirs::from("", "", "elba")
        .unwrap()
        .data_dir()
        .to_path_buf()
}

fn default_cache_dir() -> PathBuf {
    ProjectDirs::from("", "", "elba")
        .unwrap()
        .cache_dir()
        .to_path_buf()
}

impl Default for Directories {
    fn default() -> Self {
        Directories {
            bin: default_bin_dir(),
            data: default_data_dir(),
            cache: default_cache_dir(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Backend {
    pub name: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
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
