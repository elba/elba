//! Data structures related to packages.

pub mod ipkg;
pub mod lockfile;
pub mod manifest;

use crate::{
    remote::resolution::Resolution,
    util::error::{Error, Result},
};
use failure::{bail, format_err};
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};

/// Struct `Name` represents the name of a package. All packages in elba are namespaced, so all
/// packages have to have a group (pre-slash) and a name (post-slash).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name {
    inner: Arc<NameInner>,
}

#[derive(Clone, Debug, PartialOrd, Ord)]
struct NameInner {
    /// The serialized form of a name, as specified by the user
    serialization: String,
    /// The normalized form of a name for use in internal comparisons to see if two names are the
    /// same.
    ///
    /// This is separated from the serialization because the serialization is a convenience to
    /// avoid repeatedly reformatting the string when using the full group/name; it preserves
    /// all case and hyphens/underscores. The normalization, on the other hand, changes
    /// all underscores to hyphens and lowercases everything for more consistent comparison.
    ///
    /// Basically, we let the user preserve whatever pretty-pretty formatting they want, but
    /// internally, elba doesn't care.
    normalization: String,
    group: String,
    name: String,
}

impl Name {
    pub fn new(group: String, name: String) -> Result<Self> {
        let group_valid = group
            .chars()
            .all(|x| x.is_alphanumeric() || x == '_' || x == '-');
        if !group_valid {
            bail!("group can only contain letters, numbers, _, and -")
        }
        let name_valid = name
            .chars()
            .all(|x| x.is_alphanumeric() || x == '_' || x == '-');
        if !name_valid {
            bail!("name can only contain letters, numbers, _, and -")
        }

        let mut s = String::with_capacity(group.len() + 2 + name.len());
        s.push_str(&group);
        s.push('/');
        s.push_str(&name);

        let mut n = String::with_capacity(group.len() + 2 + name.len());
        let pre = &group
            .to_ascii_lowercase()
            .drain(..)
            .map(|c| if c == '_' { '-' } else { c })
            .collect::<String>();
        if pre.is_empty() {
            bail!("group cannot be empty")
        }
        n.push_str(pre);

        n.push('/');

        let post = &name
            .to_ascii_lowercase()
            .drain(..)
            .map(|c| if c == '_' { '-' } else { c })
            .collect::<String>();
        if post.is_empty() {
            bail!("name cannot be empty")
        }
        n.push_str(post);

        Ok(Name {
            inner: Arc::new(NameInner {
                serialization: s,
                normalization: n,
                group,
                name,
            }),
        })
    }

    pub fn group(&self) -> &str {
        &self.inner.group
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn normalized_group(&self) -> &str {
        self.inner.normalization.splitn(2, '/').next().unwrap()
    }

    pub fn normalized_name(&self) -> &str {
        self.inner.normalization.rsplitn(2, '/').next().unwrap()
    }

    pub fn as_str(&self) -> &str {
        &self.inner.serialization
    }

    pub fn as_normalized(&self) -> &str {
        &self.inner.normalization
    }
}

impl PartialEq for NameInner {
    fn eq(&self, other: &NameInner) -> bool {
        self.normalization == other.normalization
    }
}

impl Eq for NameInner {}

impl Hash for NameInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.normalization.hash(state);
    }
}

impl FromStr for Name {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        let v: Vec<&str> = s.split('/').collect();

        if v.len() != 2 {
            return Err(format_err!(
                "name must have group and name separated by '/'"
            ))?;
        }

        let (group, name) = (v[0].to_owned(), v[1].to_owned());

        Name::new(group, name)
    }
}

impl Serialize for Name {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
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
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
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
    inner: Arc<PackageIdInner>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct PackageIdInner {
    pub name: Name,
    pub resolution: Resolution,
}

impl PackageId {
    pub fn new(name: Name, resolution: Resolution) -> Self {
        PackageId {
            inner: Arc::new(PackageIdInner { name, resolution }),
        }
    }

    pub fn name(&self) -> &Name {
        &self.inner.name
    }

    pub fn resolution(&self) -> &Resolution {
        &self.inner.resolution
    }

    /// lowkey_eq is like eq except that it's meant to be used for determining if a package is in
    /// a lockfile already.
    pub fn lowkey_eq(&self, other: &PackageId) -> bool {
        self.name() == other.name() && self.resolution().lowkey_eq(other.resolution())
    }
}

impl FromStr for PackageId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut s = s.splitn(2, '@');
        let name = s.next().unwrap();
        let url = s.next().ok_or_else(|| {
            format_err!("package id must have name and resolution, separated by '@'")
        })?;

        let name = Name::from_str(name)?;
        let resolution = Resolution::from_str(url)?;

        Ok(PackageId::new(name, resolution))
    }
}

impl fmt::Debug for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PackageId(\"{}@{}\")", self.name(), self.resolution())
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}", self.name(), self.resolution())
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for PackageId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChecksumFmt {
    Sha256,
}

impl FromStr for ChecksumFmt {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "sha256" => Ok(ChecksumFmt::Sha256),
            _ => Err(Error::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for ChecksumFmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChecksumFmt::Sha256 => write!(f, "sha256"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Checksum {
    pub fmt: ChecksumFmt,
    pub hash: String,
}

impl FromStr for Checksum {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut s = s.splitn(2, '=');
        let fmt = s.next().unwrap();
        let hash = s.next().ok_or_else(|| Error::InvalidSourceUrl)?.to_string();
        Ok(Checksum {
            fmt: fmt.parse::<ChecksumFmt>()?,
            hash,
        })
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}={}", self.fmt, self.hash)
    }
}

impl<'de> Deserialize<'de> for Checksum {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for Checksum {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Struct `Summary` defines the summarized version of a package.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct Summary {
    pub id: PackageId,
    pub version: Version,
}

impl Summary {
    pub fn new(id: PackageId, version: Version) -> Self {
        Summary { id, version }
    }

    pub fn from_comps(name: Name, res: Resolution, version: Version) -> Self {
        let pid = PackageId::new(name, res);
        Self::new(pid, version)
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn name(&self) -> &Name {
        &self.id.name()
    }

    pub fn resolution(&self) -> &Resolution {
        &self.id.resolution()
    }

    pub fn version(&self) -> &Version {
        &self.version
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} ({})",
            self.name(),
            self.version,
            self.resolution()
        )
    }
}

impl fmt::Debug for Summary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}|{}", self.name(), self.resolution(), self.version)
    }
}

/// A Spec is like a Summary, except with more optional parts.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Spec {
    pub name: Name,
    pub resolution: Option<Resolution>,
    pub version: Option<Version>,
}

impl Spec {
    pub fn matches(&self, sum: &Summary) -> bool {
        if &self.name != sum.name() {
            return false;
        }

        if let Some(res) = self.resolution.as_ref() {
            if res != sum.resolution() {
                return false;
            }
        }

        if let Some(ver) = self.version.as_ref() {
            if ver != sum.version() {
                return false;
            }
        }

        true
    }
}

impl From<Name> for Spec {
    fn from(f: Name) -> Spec {
        Spec {
            name: f,
            version: None,
            resolution: None,
        }
    }
}

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(r) = self.resolution.as_ref() {
            write!(f, "@{}", r)?;
        }
        if let Some(v) = self.version.as_ref() {
            write!(f, "|{}", v)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Spec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl FromStr for Spec {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut ver_split = s.splitn(2, '|');
        let (pre, ver) = (ver_split.next().unwrap(), ver_split.next());
        let version = if let Some(v) = ver {
            Some(Version::from_str(v)?)
        } else {
            None
        };
        let mut res_split = pre.splitn(2, '@');
        let (name, res) = (Name::from_str(res_split.next().unwrap())?, res_split.next());
        let resolution = if let Some(r) = res {
            Some(Resolution::from_str(r)?)
        } else {
            None
        };

        Ok(Spec {
            name,
            resolution,
            version,
        })
    }
}
