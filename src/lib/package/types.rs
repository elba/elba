//! Module `package/types` provides miscellaneous datatypes related to packages.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use semver::Version;

use err::*;

// TODO: Should "test" desugar to "test/test"? Should this desugar be allowed when defining the
//       name of a package?
/// Struct `Name` represents the name of a package. All packages in matic are namespaced, so all
/// packages have to have a group (pre-slash) and a name (post-slash).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Name {
    group: String,
    name: String,
}

impl Name {
    fn new(group: String, name: String) -> Self {
        Name { group, name }
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();

        if parts.len() == 2 {
            Ok(Name::new(parts[0].to_owned(), parts[1].to_owned()))
        } else {
            // TODO: A more specific error
            Err(Error::from(ErrorKind::InvalidManifestVer))
        }
    }
}

impl Serialize for Name {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let len = self.group.len() + self.name.len() + 2;
        let mut s = String::with_capacity(len);

        s.push_str(&self.group);
        s.push('/');
        s.push_str(&self.name);

        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
