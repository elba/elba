//! Configuration for Indices.

use failure::{Error, ResultExt};
use indexmap::IndexMap;
use package::resolution::IndexRes;
use std::str::FromStr;
use toml;
use util::errors::ErrorKind;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct IndexConfig {
    pub index: IndexConfInner,
}

impl FromStr for IndexConfig {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        toml::from_str(raw)
            .context(ErrorKind::InvalidIndex)
            .map_err(Error::from)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IndexConfInner {
    pub secure: bool,
    pub dependencies: IndexMap<String, IndexRes>,
}

impl Default for IndexConfInner {
    fn default() -> Self {
        IndexConfInner {
            secure: false,
            dependencies: indexmap!(),
        }
    }
}
