//! Module `package` deals with all of the configuration files at the package level.

pub mod lockfile;
pub mod manifest;

use failure::ResultExt;
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, str::FromStr};
use url::Url;
use url_serde;

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
    pub fn new(group: String, name: String) -> Self {
        Name { group, name }
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Vec<&str> = s.splitn(2, '/').collect();

        let (group, name) = (v[0].to_owned(), v[1].to_owned());

        Ok(Name { group, name })
    }
}

impl Serialize for Name {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
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

pub type Hash = String;

/// Enum `Source` represents the possible places from which a package originated.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "source")]
pub enum Source {
    /// Git: the package originated from a git repository.
    Git {
        #[serde(with = "url_serde")]
        url: Url, /* TODO More */
    },
    /// Registry: the package originated from a Registry of some sort (can be local or remote)
    Local {
        #[serde(with = "url_serde")]
        url: Url,
    },
    /// Registry: the package originated from a Registry of some sort (can be local or remote)
    Registry {
        #[serde(with = "url_serde")]
        url: Url, /* TODO More */
    },
}

impl Source {
    /// Method `Source::from_url` parses an origin url and turns it into an Origin identifier.
    pub fn from_url(url: &str) -> Result<Self, Error> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "git" => unimplemented!(),
            "local" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Source::Local { url })
            }
            "registry" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Source::Registry { url })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl FromStr for Source {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Source::from_url(s)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageId {
    name: Name,
    version: Version,
    src: Source,
}

/// The summarized version of a package.
#[derive(Debug, Deserialize, Serialize)]
pub struct Summary {
    #[serde(flatten)]
    id: PackageId,
    version: Version,
    hash: Hash,
}
