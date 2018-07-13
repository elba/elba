use err::{Error, ErrorKind};
use failure::ResultExt;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GitTag {
    Commit(String),
    Tag(String),
}

/// The possible places from which a package can be resolved.
///
/// There are two main sources from which a package can originate: a Direct source (a path or a
/// tarball online or a git repo) and an Index (an indirect source which accrues metadata about
/// Direct sources
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Eq, Hash)]
#[serde(untagged)]
pub enum Resolution {
    Direct(DirectRes),
    Index(IndexRes),
    Root,
}

impl From<DirectRes> for Resolution {
    fn from(i: DirectRes) -> Self {
        Resolution::Direct(i)
    }
}

impl From<IndexRes> for Resolution {
    fn from(i: IndexRes) -> Self {
        Resolution::Index(i)
    }
}

impl FromStr for Resolution {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let direct = DirectRes::from_str(s);
        if s == "root" {
            Ok(Resolution::Root)
        } else if direct.is_ok() {
            direct.map(Resolution::Direct)
        } else {
            IndexRes::from_str(s).map(Resolution::Index)
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Resolution::Direct(d) => write!(f, "{}", d),
            Resolution::Index(i) => write!(f, "{}", i),
            Resolution::Root => write!(f, "root"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DirectRes {
    /// Git: the package originated from a git repository.
    Git {
        repo: Url,
        sub_path: String,
        tag: GitTag,
    },
    /// Dir: the package is on disk in a folder directory.
    Dir { url: Url },
    /// Tar: the package is an archive stored somewhere.
    Tar { url: Url },
}

impl FromStr for DirectRes {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "git" => unimplemented!(),
            "dir" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                if url.scheme() != "file" {
                    return Err(ErrorKind::InvalidSourceUrl)?;
                }
                Ok(DirectRes::Dir { url })
            }
            "tar" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(DirectRes::Tar { url })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for DirectRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DirectRes::Git {
                repo: _repo,
                sub_path: _sub_path,
                tag: _tag,
            } => unimplemented!(),
            DirectRes::Dir { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 5);
                s.push_str("dir+");
                s.push_str(url);
                write!(f, "{}", s)
            }
            DirectRes::Tar { url } => {
                let url = url.as_str();
                let mut s = String::with_capacity(url.len() + 10);
                s.push_str("dir+");
                s.push_str(url);
                write!(f, "{}", s)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IndexRes {
    pub url: Url,
}

impl FromStr for IndexRes {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "index" => {
                let url = Url::parse(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(IndexRes { url })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for IndexRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let url = self.url.as_str();
        let mut s = String::with_capacity(url.len() + 10);
        s.push_str("index+");
        s.push_str(url);
        write!(f, "{}", s)
    }
}

impl Serialize for DirectRes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DirectRes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for IndexRes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for IndexRes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
