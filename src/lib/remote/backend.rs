//! Package repositories that can be published/yanked to.
//! elba reads from indices and writes to repos.

use package::Name;
use reqwest::Client;
use semver::Version;
use std::{fs::File, time::Duration};
use url::Url;
use url_serde;
use util::errors::Res;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Backend(#[serde(with = "url_serde")] pub Url);

impl Backend {
    pub fn yank(&self, name: &Name, version: &Version, token: &str) -> Res<()> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let mut resp = client
            .post(self.0.join("package/yank").unwrap())
            .form(&[
                ("package_group_name", name.normalized_group()),
                ("package_name", name.normalized_name()),
                ("semver", &version.to_string()),
                ("yanked", "true"),
                ("token", token),
            ]).send()?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_err!("{}", resp.text()?))?
        }
    }

    pub fn publish(&self, tar: File, name: &Name, version: &Version, token: &str) -> Res<()> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let mut resp = client
            .post(self.0.join("package/publish").unwrap())
            .body(tar)
            .form(&[
                ("package_group_name", name.normalized_group()),
                ("package_name", name.normalized_name()),
                ("semver", &version.to_string()),
                ("token", token),
            ]).send()?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format_err!("{}", resp.text()?))?
        }
    }
}
