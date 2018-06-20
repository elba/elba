//! Module `package` deals with all of the configuration files at the package level.

pub mod lockfile;
pub mod manifest;

use failure::ResultExt;
use semver::{Version, VersionReq};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use url::Url;
use url_serde;

use err::*;

// TODO: Should "test" desugar to "test/test"? Should this desugar be allowed when defining the
//       name of a package?
// TODO: Legal characters?
/// Struct `Name` represents the name of a package. All packages in matic are namespaced, so all
/// packages have to have a group (pre-slash) and a name (post-slash).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name {
    /// The serialized form of a name: "group/name"
    serialization: String,
    group: String,
    name: String,
}

impl Name {
    pub fn new(group: String, name: String) -> Self {
        let mut s = String::with_capacity(group.len() + 2 + name.len());
        s.push_str(&group);
        s.push('/');
        s.push_str(&name);
        Name {
            serialization: s,
            group,
            name,
        }
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn as_str(&self) -> &str {
        &self.serialization
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Vec<&str> = s.split('/').collect();

        if v.len() != 2 {
            return Err(ErrorKind::InvalidPackageId)?;
        }

        let (group, name) = (v[0].to_owned(), v[1].to_owned());

        Ok(Name {
            serialization: s.to_owned(),
            group,
            name,
        })
    }
}

impl Serialize for Name {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.serialization)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl AsRef<str> for Name {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.serialization
    }
}

/// Struct `Dep` represents a requirement or a dependency.
#[derive(Debug, Deserialize, Hash, Serialize)]
pub struct Dep {
    name: Name,
    req: VersionReq,
}

/// Enum `Source` represents the possible places from which a package originated.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "source")]
pub enum Source {
    /// Git: the package originated from a git repository.
    Git {
        #[serde(with = "url_serde")]
        url: Url, /* TODO More */
    },
    /// Dir: the package is on disk in a folder directory.
    Dir {
        #[serde(with = "url_serde")]
        url: Url,
    },
    /// Index: the package was resolved from an index (can be local or remote).
    Index {
        #[serde(with = "url_serde")]
        url: Url, /* TODO More */
    },
}

impl FromStr for Source {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "git" => unimplemented!(),
            "dir" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Source::Dir { url })
            }
            "index" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Source::Index { url })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Source::Git { url } => unimplemented!(),
            Source::Dir { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 5);
                s.push_str("dir+");
                s.push_str(url);
                write!(f, "{}", s)
            }
            Source::Index { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 10);
                s.push_str("index+");
                s.push_str(url);
                write!(f, "{}", s)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageId {
    name: Name,
    version: Version,
    source: Source,
}

impl PackageId {
    pub fn new(name: Name, version: Version, source: Source) -> Self {
        PackageId {
            name,
            version,
            source,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn source(&self) -> &Source {
        &self.source
    }
}

impl FromStr for PackageId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next().ok_or_else(|| ErrorKind::InvalidPackageId)?;
        let url = s.next().ok_or_else(|| ErrorKind::InvalidPackageId)?;

        let name = Name::from_str(name)?;
        let version = Version::parse(version).context(ErrorKind::InvalidPackageId)?;
        let source = Source::from_str(url)?;

        Ok(PackageId {
            name,
            version,
            source,
        })
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for PackageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let name = self.name.as_str();
        let vers = &self.version.to_string();
        let src = &self.source.to_string();

        let mut s = String::with_capacity(name.len() + vers.len() + src.len() + 5);

        s.push_str(name);
        s.push(' ');
        s.push_str(vers);
        s.push(' ');
        s.push_str(src);

        serializer.serialize_str(&s)
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumFmt {
    Sha512,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Checksum {
    fmt: ChecksumFmt,
    checksum: String,
}

/// Struct `Summary` defines the summarized version of a package.
///
/// The type parameter `T` allows us to use this struct for both resolved and unresolved
/// dependencies.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct Summary<T> {
    id: PackageId,
    checksum: Checksum,
    #[serde(default = "Vec::new")]
    dependencies: Vec<T>,
}

impl<T> Summary<T> {
    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn checksum(&self) -> &Checksum {
        &self.checksum
    }

    pub fn dependencies(&self) -> &[T] {
        &self.dependencies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO
}
