//! Package repositories that can be published/yanked to.
//! elba reads from indices and writes to repos.

use crate::{package::Name, remote::Indices, util::errors::Res};
use failure::format_err;
use reqwest::Client;
use semver::Version;
use serde_derive::{Deserialize, Serialize};
use std::{fs::File, time::Duration};
use url::{
    percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET},
    Url,
};
use url_serde;

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

pub struct SearchVersioned {
    pub group: String,
    pub name: String,
    pub version: Version,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Backend(#[serde(with = "url_serde")] pub Url);

impl Backend {
    pub fn yank(&self, name: &Name, version: &Version, token: &str) -> Res<()> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let mut resp = client
            .patch(
                self.0
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
                self.0
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
            .get(self.0.join("api/v1/packages/search").unwrap())
            .query(&[("q", query)])
            .send()?;

        if resp.status().is_success() {
            let sr = resp.json::<SearchResponse<SearchPkg>>()?;

            let packages = sr.packages.into_iter().map(|x| {
                let n = Name::new(x.group.clone(), x.name.clone()).unwrap();
                let sum = indices.select_by_spec(&n.into())?;
                Ok(SearchVersioned {
                    group: x.group,
                    name: x.name,
                    version: sum.version,
                })
            }).collect::<Res<_>>()?;

            let ns = SearchResponse {
                packages,
            };

            Ok(ns)
        } else {
            Err(format_err!("{} {}", resp.status(), resp.text()?))?
        }
    }
}
