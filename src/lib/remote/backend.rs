//! Package repositories that can be published/yanked to.
//! elba reads from indices and writes to repos.

use url::Url;
use url_serde;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Backend(#[serde(with = "url_serde")] pub Url);
