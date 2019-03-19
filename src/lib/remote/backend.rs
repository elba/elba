//! Package repositories that can be published/yanked to.
//! elba reads from indices and writes to repos.

use crate::{package::Name, util::errors::Res};
use failure::format_err;
use reqwest::Client;
use semver::Version;
use serde_derive::{Deserialize, Serialize};
use std::{fs::File, time::Duration};
use url::Url;
use url_serde;

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
        let mut resp = client
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

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_err!("{}", resp.text()?))?
        }
    }
}
