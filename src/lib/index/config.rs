//! Configuration for Indices.

use package::IndexRes;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct IndexConfig {
    index: IndexConfInner,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct IndexConfInner {
    secure: bool,
    dependencies: Vec<IndexRes>,
}

impl Default for IndexConfInner {
    fn default() -> Self {
        IndexConfInner {
            secure: false,
            dependencies: vec![],
        }
    }
}
