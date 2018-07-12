//! Data structures related to packages.

pub mod lockfile;
pub mod manifest;
pub mod resolution;
pub mod version;

use self::resolution::Resolution;
use err::*;
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, rc::Rc, str::FromStr};

// TODO: Should "test" desugar to "test/test"? Should this desugar be allowed when defining the
//       name of a package?
// TODO: Legal characters?
// TODO: Treat hyphens and underscores the same?
/// Struct `Name` represents the name of a package. All packages in elba are namespaced, so all
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

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner.serialization)
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PackageId {
    pub name: Name,
    pub resolution: Resolution,
}

impl PackageId {
    pub fn new(name: Name, resolution: Resolution) -> Self {
        PackageId { name, resolution }
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

        Ok(PackageId { name, resolution })
    }
}

impl fmt::Debug for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PackageId(\"{} {}\")", self.name, self.resolution)
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.name, self.resolution)
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
        serializer.serialize_str(&self.to_string())
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
        Summary { id, version }
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
