//! Module `package` deals with all of the configuration files at the package level.

pub mod lockfile;
pub mod manifest;
pub mod version;

use self::version::Constraint;
use err::*;
use failure::ResultExt;
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, rc::Rc, str::FromStr};
use url::Url;
use url_serde;

// TODO: Should "test" desugar to "test/test"? Should this desugar be allowed when defining the
//       name of a package?
// TODO: Legal characters?
// TODO: Treat hyphens and underscores the same?
/// Struct `Name` represents the name of a package. All packages in matic are namespaced, so all
/// packages have to have a group (pre-slash) and a name (post-slash).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name {
    inner: Rc<NameInner>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct NameInner {
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
            inner: Rc::new(NameInner {
                serialization: s,
                group,
                name,
            }),
        }
    }

    pub fn group(&self) -> &str {
        &self.inner.group
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn as_str(&self) -> &str {
        &self.inner.serialization
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

        Ok(Name::new(group, name))
    }
}

impl Serialize for Name {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
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
        self.as_str()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum GitTag {
    Commit(String),
    Tag(String),
}

// TODO: Custom (de)serialization?
/// Enum `Resolution` represents the possible places from which a package can be resolved. A package
/// can be manually set to be located in a git repo or a local file directory, or it can be
/// resolved with a package index.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// Git: the package originated from a git repository.
    Git {
        repo: Url,
        tag: GitTag,
    },
    /// Dir: the package is on disk in a folder directory.
    Dir {
        url: Url,
    },
    /// Tar: the package originated from an archive stored somewhere.
    Tar {
        url: Url,
    },
    /// Index: the package was resolved from an index (can be local or remote).
    Index {
        url: Url, /* TODO More */
    },
}

impl FromStr for Resolution {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "git" => unimplemented!(),
            "dir" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Resolution::Dir { url })
            }
            "tar" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Resolution::Tar { url })
            }
            "index" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(Resolution::Index { url })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Resolution::Git {
                repo: _repo,
                tag: _tag,
            } => unimplemented!(),
            Resolution::Dir { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 5);
                s.push_str("dir+");
                s.push_str(url);
                write!(f, "{}", s)
            }
            Resolution::Index { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 10);
                s.push_str("index+");
                s.push_str(url);
                write!(f, "{}", s)
            }
            Resolution::Tar { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 10);
                s.push_str("tar+");
                s.push_str(url);
                write!(f, "{}", s)
            }
        }
    }
}

impl Serialize for Resolution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Resolution {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageId {
    pub name: Name,
    pub resolution: Resolution,
}

impl PackageId {
    pub fn new(name: Name, resolution: Resolution) -> Self {
        PackageId {
            name,
            resolution,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn resolution(&self) -> &Resolution {
        &self.resolution
    }
}

impl FromStr for PackageId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.splitn(2, ' ');
        let name = s.next().unwrap();
        let url = s.next().ok_or_else(|| ErrorKind::InvalidPackageId)?;

        let name = Name::from_str(name)?;
        let resolution = Resolution::from_str(url)?;

        Ok(PackageId {
            name,
            resolution,
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
        let src = &self.resolution.to_string();

        let mut s = String::with_capacity(name.len() + src.len() + 5);

        s.push_str(name);
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
    hash: String,
}

// TODO: Should Summaries have checksums?
/// Struct `Summary` defines the summarized version of a package.
///
/// The type parameter `T` allows us to use this struct for both resolved and unresolved
/// dependencies.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct Summary {
    pub id: PackageId,
    pub version: Version,
}

impl Summary {
    pub fn new(id: PackageId, version: Version) -> Self {
        Summary {
            id,
            version,
        }
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn name(&self) -> &Name {
        &self.id.name
    }

    pub fn version(&self) -> &Version {
        &self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO
}
