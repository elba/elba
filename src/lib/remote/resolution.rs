use std::io;

use std::{fmt, fs, io::BufReader, path::PathBuf, str::FromStr};

use failure::{bail, format_err, ResultExt};
use flate2::read::GzDecoder;
use git2::{BranchType, Repository, Sort};
use reqwest::blocking::Client;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use tar::Archive;
use url::Url;

use crate::{
    package::Checksum,
    util::{
        clear_dir,
        error::{Error, Result},
        git::{clone, fetch, reset, update_submodules},
        lock::DirLock,
    },
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
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self> {
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
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Resolution {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Resolution {
    pub fn direct(&self) -> Option<&DirectRes> {
        if let Resolution::Direct(d) = &self {
            Some(&d)
        } else {
            None
        }
    }

    pub fn is_tar(&self) -> bool {
        if let Resolution::Direct(d) = &self {
            d.is_tar()
        } else {
            false
        }
    }

    pub fn is_git(&self) -> bool {
        if let Resolution::Direct(d) = &self {
            d.is_git()
        } else {
            false
        }
    }

    pub fn is_dir(&self) -> bool {
        if let Resolution::Direct(d) = &self {
            d.is_dir()
        } else {
            false
        }
    }

    pub fn lowkey_eq(&self, other: &Resolution) -> bool {
        match (self, other) {
            (Resolution::Direct(d), Resolution::Direct(d2)) => d.lowkey_eq(d2),
            (Resolution::Index(i), Resolution::Index(i2)) => i == i2,
            (_, _) => false,
        }
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
    pub fn lowkey_eq(&self, other: &DirectRes) -> bool {
        match (self, other) {
            (DirectRes::Git { repo: r1, .. }, DirectRes::Git { repo: r2, .. }) => r1 == r2,
            _ => self == other,
        }
    }
}

/// Retrieves a package in the form of a tarball.
fn retrieve_tar(
    url: Url,
    client: &Client,
    target: &DirLock,
    cksum: Option<&Checksum>,
) -> Result<()> {
    let mut resp = client
        .get(url.as_str())
        .send()
        .and_then(|resp| resp.error_for_status())?;

    let mut buf: Vec<u8> = vec![];
    resp.copy_to(&mut buf)?;

    let hash = hex::encode(Sha256::digest(&buf[..]).as_slice());
    if let Some(cksum) = cksum {
        if cksum.hash != hash {
            bail!(format_err!("tarball checksum doesn't match real checksum"));
        }
    }

    let archive = GzDecoder::new(&buf[..]);
    let mut archive = Archive::new(archive);

    clear_dir(target.path())?;

    archive.unpack(target.path())?;

    Ok(())
}

impl DirectRes {
    pub fn retrieve(
        &self,
        client: &Client,
        target: &DirLock,
        eager: bool,
        dl_f: impl Fn(bool) -> Result<()>,
    ) -> Result<Option<DirectRes>> {
        match self {
            DirectRes::Tar { url, cksum } => match url.scheme() {
                "http" | "https" => {
                    dl_f(true)?;
                    retrieve_tar(url.clone(), &client, &target, cksum.as_ref())
                        .context(Error::CannotDownload)?;

                    Ok(None)
                }
                "file" => {
                    dl_f(false)?;
                    let mut archive =
                        fs::File::open(target.path()).context(Error::CannotDownload)?;

                    let mut hash = Sha256::new();
                    io::copy(&mut archive, &mut hash).context(Error::CannotDownload)?;
                    let hash = hex::encode(hash.result());

                    if let Some(cksum) = cksum {
                        if cksum.hash != hash {
                            return Err(format_err!(
                                "tarball checksum doesn't match real checksum"
                            ))?;
                        }
                    }

                    let archive = BufReader::new(archive);
                    let archive = GzDecoder::new(archive);
                    let mut archive = Archive::new(archive);

                    clear_dir(target.path())?;

                    archive
                        .unpack(target.path())
                        .context(Error::CannotDownload)?;

                    Ok(None)
                }
                _ => unreachable!(),
            },
            DirectRes::Git { repo: url, tag } => {
                // If we find a directory which already has a repo, we just check out the correct
                // version of it. Whether or not a new dir is created isn't our job, that's for the
                // Cache. If the Cache points to a directory that already exists, it means that the
                // branch data or w/e is irrelevant.
                let repo = Repository::open(target.path());
                let repo = match repo {
                    Ok(r) => {
                        let mut repo = r;
                        // This logic is for in case we are pointed to an existing git repository.
                        // We only want to NOT update an existing git repository if eager is false.
                        // We assume that the HEAD of the repo is at the current "locked" state.
                        //
                        // If the tag is a branch:
                        if !eager {
                            if let Ok(b) = repo.find_branch(&tag, BranchType::Local) {
                                let head = b.into_reference().resolve()?.peel_to_commit()?;
                                let cur = repo.head()?.resolve()?.peel_to_commit()?;

                                let mut revwalk = repo.revwalk()?;
                                revwalk.push(head.id())?;
                                revwalk.set_sorting(Sort::TOPOLOGICAL)?;

                                if revwalk.any(|x| x == Ok(cur.id())) {
                                    if &cur.id().to_string() == tag {
                                        return Ok(None);
                                    } else {
                                        return Ok(Some(DirectRes::Git {
                                            repo: url.clone(),
                                            tag: cur.id().to_string(),
                                        }));
                                    }
                                }
                            }

                            // Otherwise, if the tag is an exact pointer to a commit, we try to check out to
                            // it locally without fetching anything
                            let target =
                                repo.revparse_single(&tag).and_then(|x| x.peel_to_commit());
                            let cur = repo
                                .head()
                                .and_then(|x| x.resolve())
                                .and_then(|x| x.peel_to_commit());
                            if let Ok(t) = target {
                                if let Ok(c) = cur {
                                    if t.id() == c.id() {
                                        if tag == &c.id().to_string() {
                                            return Ok(None);
                                        } else {
                                            return Ok(Some(DirectRes::Git {
                                                repo: url.clone(),
                                                tag: c.id().to_string(),
                                            }));
                                        }
                                    } else {
                                        // Because we know the other tag exists in our local copy of the
                                        // repo, we can just check out into that and return
                                        let obj = t.into_object().clone();
                                        reset(&repo, &obj).with_context(|e| {
                                            format_err!(
                                                "couldn't checkout commit {}: {}",
                                                obj.id(),
                                                e
                                            )
                                        })?;
                                        if tag == &obj.id().to_string() {
                                            return Ok(None);
                                        } else {
                                            return Ok(Some(DirectRes::Git {
                                                repo: url.clone(),
                                                tag: obj.id().to_string(),
                                            }));
                                        }
                                    }
                                }
                            }
                        }

                        // Get everything!!
                        dl_f(true)?;
                        let refspec = "refs/heads/*:refs/heads/*";
                        fetch(&mut repo, &url, refspec).with_context(|e| {
                            format_err!("couldn't fetch git repo {}: {}", url, e)
                        })?;
                        repo
                    }
                    Err(_) => {
                        clear_dir(target.path())?;
                        dl_f(true)?;
                        clone(url, target.path()).with_context(|e| {
                            format_err!("couldn't fetch git repo {}:\n{}", url, e)
                        })?
                    }
                };

                let obj = repo.revparse_single(&tag).context(Error::CannotDownload)?;
                reset(&repo, &obj)
                    .with_context(|e| format_err!("couldn't fetch git repo {}:\n{}", url, e))?;
                update_submodules(&repo).with_context(|e| {
                    format_err!("couldn't update submodules for git repo {}:\n{}", url, e)
                })?;

                let id = obj.peel_to_commit()?.id().to_string();

                Ok(Some(DirectRes::Git {
                    repo: url.clone(),
                    tag: id,
                }))
            }
            DirectRes::Dir { path } => {
                // If this package is located on disk, we don't have to do anything...
                dl_f(false)?;
                if path.is_relative() {
                    bail!(format_err!(
                        "can't retrieve dir resolution by relative path {}",
                        path.display()
                    ))
                }
                if path.exists() {
                    Ok(None)
                } else {
                    bail!(format_err!("can't find directory {}", path.display()))
                }
            }
        }
    }

    pub fn is_tar(&self) -> bool {
        if let DirectRes::Tar { .. } = &self {
            true
        } else {
            false
        }
    }

    pub fn is_git(&self) -> bool {
        if let DirectRes::Git { .. } = &self {
            true
        } else {
            false
        }
    }

    pub fn is_dir(&self) -> bool {
        if let DirectRes::Dir { .. } = &self {
            true
        } else {
            false
        }
    }
}

impl FromStr for DirectRes {
    type Err = failure::Error;

    fn from_str(url: &str) -> Result<Self> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let rest = parts.next().ok_or_else(|| Error::InvalidSourceUrl)?;

        match utype {
            "git" => {
                let mut url = Url::parse(rest).context(Error::InvalidSourceUrl)?;
                let tag = url.fragment().unwrap_or_else(|| "master").to_owned();

                url.set_fragment(None);
                Ok(DirectRes::Git { repo: url, tag })
            }
            "dir" => {
                let path = PathBuf::from(rest);
                Ok(DirectRes::Dir { path })
            }
            "tar" => {
                let mut url = Url::parse(rest).context(Error::InvalidSourceUrl)?;
                if url.scheme() != "http" && url.scheme() != "https" && url.scheme() != "file" {
                    return Err(Error::InvalidSourceUrl)?;
                }
                let cksum = url.fragment().and_then(|x| Checksum::from_str(x).ok());
                url.set_fragment(None);
                Ok(DirectRes::Tar { url, cksum })
            }
            _ => Err(Error::InvalidSourceUrl)?,
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

impl From<IndexRes> for DirectRes {
    fn from(i: IndexRes) -> Self {
        i.res
    }
}

impl FromStr for IndexRes {
    type Err = failure::Error;

    fn from_str(url: &str) -> Result<Self> {
        let mut parts = url.splitn(2, '+');
        let utype = parts.next().unwrap();
        let url = parts.next().ok_or_else(|| Error::InvalidSourceUrl)?;

        match utype {
            "index" => {
                let res = DirectRes::from_str(url).context(Error::InvalidSourceUrl)?;
                Ok(IndexRes { res })
            }
            _ => Err(Error::InvalidSourceUrl)?,
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
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DirectRes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Serialize for IndexRes {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for IndexRes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
