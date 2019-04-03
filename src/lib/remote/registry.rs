//! Package repositories that can be published/yanked to.
//! elba reads from indices and writes to repos.

use crate::{package::Name, remote::Indices, util::errors::Res};
use failure::{format_err, Error, ResultExt};
use reqwest::Client;
use semver::Version;
use serde::{ser, de};
use serde_derive::{Deserialize, Serialize};
use std::{fmt, fs::File, time::Duration, str::FromStr};
use url::{
    percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET},
    Url,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResponse<T> {
    pub packages: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchPkg {
    pub group: String,
    #[serde(rename = "package")]
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SearchVersioned {
    pub group: String,
    pub name: String,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Registry {
    pub url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegistryMeta {
    pub api: u16,
}

impl fmt::Display for Registry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl FromStr for Registry {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(url).context(format_err!("invalid registry url"))?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(format_err!("invalid scheme for registry url"))?;
        }
        Ok(Registry { url })
    }
}

impl<'de> de::Deserialize<'de> for Registry {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl ser::Serialize for Registry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Registry {
    pub fn new(url: Url) -> Self {
        Registry { url }
    }

    pub fn meta(&self) -> Res<RegistryMeta> {
        // endpoint: /api/registry_metadata
        unimplemented!()
    }

    pub fn yank(&self, name: &Name, version: &Version, token: &str) -> Res<()> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let mut resp = client
            .patch(
                self.url
                    .join(&format!(
                        "api/v1/packages/{}/{}/{}/yank",
                        name.group(),
                        name.name(),
                        &version.to_string()
                    ))
                    .unwrap(),
            )
            .query(&[("yanked", "true"), ("token", token)])
            .send()?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_err!("{}", resp.text()?))?
        }
    }

    pub fn publish(&self, tar: File, name: &Name, version: &Version, token: &str) -> Res<()> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        client
            .put(
                self.url
                    .join(&format!(
                        "api/v1/packages/{}/{}/{}/publish",
                        name.group(),
                        name.name(),
                        &version.to_string()
                    ))
                    .unwrap(),
            )
            .query(&[("token", token)])
            .body(tar)
            .send()?;

        Ok(())
    }

    pub fn search(&self, indices: &Indices, query: &str) -> Res<SearchResponse<SearchVersioned>> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        // Encode the string so that it's safe for a url
        let query = utf8_percent_encode(query, DEFAULT_ENCODE_SET).collect::<String>();
        let mut resp = client
            .get(self.url.join("api/v1/packages/search").unwrap())
            .query(&[("q", query)])
            .send()?;

        if resp.status().is_success() {
            let sr = resp.json::<SearchResponse<SearchPkg>>()?;

            let packages = sr
                .packages
                .into_iter()
                .map(|x| {
                    let n = Name::new(x.group.clone(), x.name.clone()).unwrap();
                    let sum = indices.select_by_spec(&n.into())?;
                    Ok(SearchVersioned {
                        group: x.group,
                        name: x.name,
                        version: sum.version,
                    })
                })
                .collect::<Res<_>>()?;

            let ns = SearchResponse { packages };

            Ok(ns)
        } else {
            Err(format_err!("{} {}", resp.status(), resp.text()?))?
        }
    }

    pub fn retrieve_url(&self, name: &Name, version: &Version) -> Url {
        self.url
            .join(&format!(
                "api/v1/packages/{}/{}/{}/download",
                name.group(),
                name.name(),
                &version.to_string()
            ))
            .unwrap()
    }
}
