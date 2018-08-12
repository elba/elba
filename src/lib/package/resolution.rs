use super::Checksum;
use failure::{Error, ResultExt};
use flate2::read::GzDecoder;
use git2::{ObjectType, Repository};
use reqwest::Client;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{fmt, fs, io::BufReader, path::PathBuf, str::FromStr};
use tar::Archive;
use url::Url;
use util::{
    clear_dir,
    errors::ErrorKind,
    git::{clone, fetch, reset, update_submodules},
    hexify_hash,
    lock::DirLock,
};

/// The possible places from which a package can be resolved.
///
/// There are two main sources from which a package can originate: a Direct source (a path or a
/// tarball online or a git repo) and an Index (an indirect source which accrues metadata about
/// Direct sources
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resolution {
    Direct(DirectRes),
    Index(IndexRes),
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
        if direct.is_ok() {
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
pub enum DirectRes {
    /// Git: the package originated from a git repository.
    Git { repo: Url, tag: String },
    /// Dir: the package is on disk in a folder directory.
    Dir { path: PathBuf },
    /// Tar: the package is an archive stored somewhere.
    ///
    /// Tarballs are the only direct resolution which is allowed to have a checksum; this doesn't
    /// really make sense for DirectRes::Local, and we leave validation of repositories to Git
    /// itself. Checksums are stored in the fragment of the resolution url, with they key being the
    /// checksum format.
    Tar { url: Url, cksum: Option<Checksum> },
}

impl DirectRes {
    // TODO: argument progress: impl Fn(u8) where 0 <= u8 <= 100
    pub fn retrieve(&self, client: &Client, target: &DirLock) -> Result<(), Error> {
        match self {
            DirectRes::Tar { url, cksum } => match url.scheme() {
                "http" | "https" => client
                    .get(url.clone())
                    .send()
                    .map_err(|_| Error::from(ErrorKind::CannotDownload))
                    .and_then(|mut r| {
                        let mut buf: Vec<u8> = vec![];
                        r.copy_to(&mut buf).context(ErrorKind::CannotDownload)?;

                        let hash = hexify_hash(Sha256::digest(&buf[..]).as_slice());
                        if let Some(cksum) = cksum {
                            if cksum.hash == hash {
                                return Err(ErrorKind::Checksum)?;
                            }
                        }

                        let archive = BufReader::new(&buf[..]);
                        let archive = GzDecoder::new(archive);
                        let mut archive = Archive::new(archive);

                        fs::create_dir(target.path())?;

                        archive
                            .unpack(target.path())
                            .context(ErrorKind::CannotDownload)?;

                        Ok(())
                    }),
                "file" => {
                    let mut archive =
                        fs::File::open(target.path()).context(ErrorKind::CannotDownload)?;

                    let hash = hexify_hash(
                        Sha256::digest_reader(&mut archive)
                            .context(ErrorKind::CannotDownload)?
                            .as_slice(),
                    );

                    if let Some(cksum) = cksum {
                        if cksum.hash == hash {
                            return Err(ErrorKind::Checksum)?;
                        }
                    }

                    let archive = BufReader::new(archive);
                    let archive = GzDecoder::new(archive);
                    let mut archive = Archive::new(archive);

                    archive
                        .unpack(target.path())
                        .context(ErrorKind::CannotDownload)?;

                    Ok(())
                }
                _ => Err(Error::from(ErrorKind::CannotDownload)),
            },
            DirectRes::Git { repo: url, tag } => {
                // If we find a directory which already has a repo, we just check out the correct
                // version of it. Whether or not a new dir is created isn't our job, that's for the
                // Cache. If the Cache points to a directory that already exists, it means that the
                // branch data or w/e is irrelevant.
                let repo = Repository::open(target.path());

                let mut repo = match repo {
                    Ok(r) => r,
                    Err(_) => {
                        clear_dir(target.path())?;
                        clone(url, target.path()).with_context(|e| {
                            format_err!("couldn't fetch git repo {}:\n{}", url, e)
                        })?
                    }
                };

                // Get everything!!
                let refspec = "refs/heads/*:refs/heads/*";
                fetch(&mut repo, &url, refspec)
                    .with_context(|e| format_err!("couldn't fetch git repo {}:\n{}", url, e))?;
                let head = repo.head()?.resolve()?.peel(ObjectType::Any)?;
                reset(&repo, &head)
                    .with_context(|e| format_err!("couldn't fetch git repo {}:\n{}", url, e))?;
                update_submodules(&repo).with_context(|e| {
                    format_err!("couldn't update submodules for git repo {}:\n{}", url, e)
                })?;

                let branch = tag;

                let obj = repo
                    .revparse_single(&branch)
                    .context(ErrorKind::CannotDownload)?;
                repo.checkout_tree(&obj, None)
                    .context(ErrorKind::CannotDownload)?;

                Ok(())
            }
            DirectRes::Dir { path: _path } => {
                // If this package is located on disk, we don't have to do anything...
                Ok(())
            }
        }
    }
}

impl FromStr for DirectRes {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let rest = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "git" => {
                let mut url = Url::parse(rest).context(ErrorKind::InvalidSourceUrl)?;
                let tag = url.fragment().unwrap_or_else(|| "master").to_owned();

                url.set_fragment(None);
                Ok(DirectRes::Git { repo: url, tag })
            }
            "dir" => {
                let path = PathBuf::from(rest);
                Ok(DirectRes::Dir { path })
            }
            "tar" => {
                let mut url = Url::parse(rest).context(ErrorKind::InvalidSourceUrl)?;
                if url.scheme() != "http" && url.scheme() != "https" && url.scheme() != "file" {
                    return Err(ErrorKind::InvalidSourceUrl)?;
                }
                let cksum = url.fragment().and_then(|x| Checksum::from_str(x).ok());
                url.set_fragment(None);
                Ok(DirectRes::Tar { url, cksum })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for DirectRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DirectRes::Git { repo, tag } => write!(f, "git+{}#{}", repo, tag),
            DirectRes::Dir { path } => write!(f, "dir+{}", path.display()),
            DirectRes::Tar { url, cksum } => {
                let url = url.as_str();
                write!(
                    f,
                    "tar+{}{}",
                    url,
                    if let Some(cksum) = cksum {
                        "#".to_string() + &cksum.to_string()
                    } else {
                        "".to_string()
                    },
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IndexRes {
    pub res: DirectRes,
}

impl From<DirectRes> for IndexRes {
    fn from(d: DirectRes) -> Self {
        IndexRes { res: d }
    }
}

impl FromStr for IndexRes {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| ErrorKind::InvalidSourceUrl)?;

        match utype {
            "index" => {
                let res = DirectRes::from_str(url).context(ErrorKind::InvalidSourceUrl)?;
                Ok(IndexRes { res })
            }
            _ => Err(ErrorKind::InvalidSourceUrl)?,
        }
    }
}

impl fmt::Display for IndexRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let url = self.res.to_string();
        let mut s = String::with_capacity(url.len() + 10);
        s.push_str("index+");
        s.push_str(&url);
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
