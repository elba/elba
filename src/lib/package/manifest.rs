//! Package manifest files.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use failure::{format_err, Error, ResultExt};
use ignore::gitignore::GitignoreBuilder;
use indexmap::IndexMap;
use semver::Version;
use semver_constraints::Constraint;
use serde::Deserialize;
use toml;
use url::Url;
use walkdir::{DirEntry, WalkDir};

use super::*;
use crate::{
    remote::resolution::{DirectRes, IndexRes},
    util::{valid_file, SubPath},
};

// TODO: Package aliasing. Have dummy alias files in the root target folder.
//
// e.g. to alias `me/lightyear` with default root module `Me.Lightyear` as the module
// `Yeet.Lightyeet`, in the target folder, we make the following file in the proper directory
// (directory won't matter for Blodwen/Idris 2):
//
// ```idris
// module Yeet.Lightyeet
//
// import public Me.Lightyear
// ```
//
// Behind the scenes, we build this as its own package with the package it's aliasing as
// its only dependency, throw it in the global cache, and add this to the import dir of the root
// package instead of the original.
//
// With this in place, we can safely avoid module namespace conflicts.

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Manifest {
    pub package: PackageInfo,
    #[serde(default = "IndexMap::new")]
    pub dependencies: IndexMap<Name, DepReq>,
    #[serde(default = "IndexMap::new")]
    pub dev_dependencies: IndexMap<Name, DepReq>,
    #[serde(default)]
    pub targets: Targets,
    #[serde(default)]
    pub workspace: IndexMap<Name, SubPath>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
}

impl Manifest {
    // Returns only the workspace portion of a manifest.
    pub fn workspace(s: &str) -> Option<IndexMap<Name, SubPath>> {
        toml::value::Value::try_from(&s)
            .ok()?
            .get("workspace")?
            .clone()
            .try_into()
            .ok()
    }

    pub fn version(&self) -> &Version {
        &self.package.version
    }

    pub fn name(&self) -> &Name {
        &self.package.name
    }

    pub fn deps(
        &self,
        ixmap: &IndexMap<String, IndexRes>,
        parent_pkg: &PackageId,
        dev_deps: bool,
    ) -> Result<IndexMap<PackageId, Constraint>> {
        let mut deps = IndexMap::new();
        for (n, dep) in &self.dependencies {
            let dep = dep.clone();
            let (pid, c) = dep.into_dep(ixmap, parent_pkg, n.clone())?;
            deps.insert(pid, c);
        }

        if dev_deps {
            for (n, dep) in &self.dev_dependencies {
                let dep = dep.clone();
                let (pid, c) = dep.into_dep(ixmap, parent_pkg, n.clone())?;
                deps.insert(pid, c);
            }
        }

        Ok(deps)
    }

    pub fn list_files<P>(
        &self,
        pkg_root: &Path,
        search_root: &Path,
        mut p: P,
    ) -> Result<impl Iterator<Item = DirEntry>>
    where
        P: FnMut(&DirEntry) -> bool,
    {
        let mut excludes = GitignoreBuilder::new(pkg_root);
        if let Some(rs) = self.package.exclude.as_ref() {
            for r in rs {
                excludes.add_line(None, r)?;
            }
        }
        if pkg_root.join(".gitignore").exists() {
            if let Some(e) = excludes.add(pkg_root.join(".gitignore")) {
                return Err(e)?;
            }
        }
        let excludes = excludes
            .build()
            .with_context(|e| format_err!("invalid excludes: {}", e))?;

        let walker = WalkDir::new(search_root)
            .follow_links(true)
            .into_iter()
            .filter_entry(move |x| {
                !excludes
                    .matched_path_or_any_parents(x.path(), x.file_type().is_dir())
                    .is_ignore()
                    && p(&x)
            })
            .filter_map(|x| {
                x.ok()
                    .and_then(|x| if valid_file(&x) { Some(x) } else { None })
            });

        Ok(walker)
    }

    pub fn validate(&self) -> Result<()> {
        if self
            .package
            .description
            .as_ref()
            .filter(|description| description.len() > 244)
            .is_some()
        {
            bail!(format_err!("descrption is over 244 characters"));
        }
        if self
            .package
            .license
            .as_ref()
            .filter(|license| license.len() > 20)
            .is_some()
        {
            bail!(format_err!("license is over 20 characters"));
        }
        if self.package.keywords.len() > 5 {
            bail!(format_err!("keywords should no more than 5"));
        }
        if self
            .package
            .keywords
            .iter()
            .any(|keyword| keyword.trim().is_empty())
        {
            bail!(format_err!("one of the keywords is empty"))
        }
        if self
            .package
            .keywords
            .iter()
            .any(|keyword| keyword.split_whitespace().skip(1).next().is_some())
        {
            bail!(format_err!("one of the keywords contains whitespace"));
        }
        Ok(())
    }
}

impl FromStr for Manifest {
    type Err = failure::Error;

    fn from_str(raw: &str) -> Result<Self> {
        let toml: Manifest = toml::from_str(raw)
            .with_context(|e| format_err!("invalid manifest file: {}", e))
            .map_err(Error::from)?;
        toml.validate()?;
        Ok(toml)
    }
}

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PackageInfo {
    pub name: Name,
    pub version: Version,
    pub authors: Vec<String>,
    pub description: Option<String>,
    #[serde(default = "Vec::new")]
    pub keywords: Vec<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub readme: Option<SubPath>,
    pub license: Option<String>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum DepReq {
    Registry(Constraint),
    RegLong {
        version: Constraint,
        index: String,
    },
    Local {
        path: PathBuf,
    },
    Git {
        git: Url,
        #[serde(default = "default_tag")]
        tag: String,
    },
}

fn default_tag() -> String {
    "master".to_owned()
}

impl DepReq {
    pub fn into_dep(
        self,
        ixmap: &IndexMap<String, IndexRes>,
        parent_pkg: &PackageId,
        n: Name,
    ) -> Result<(PackageId, Constraint)> {
        match self {
            DepReq::Registry(c) => {
                let def_index = ixmap
                    .get_index(0)
                    .ok_or_else(|| format_err!("no default index"))?;
                let pi = PackageId::new(n, def_index.1.clone().into());
                Ok((pi, c))
            }
            DepReq::RegLong { version, index } => {
                if let Some(mapped) = ixmap.get(&index) {
                    let pi = PackageId::new(n, mapped.clone().into());
                    Ok((pi, version))
                } else {
                    let ix = IndexRes::from_str(&index)?;
                    let pi = PackageId::new(n, ix.into());
                    Ok((pi, version))
                }
            }
            DepReq::Local { path } => {
                if let &Resolution::Direct(DirectRes::Dir { path: parent_root }) =
                    &parent_pkg.resolution()
                {
                    let res = DirectRes::Dir {
                        path: parent_root.join(path),
                    };
                    let pi = PackageId::new(n, res.into());
                    Ok((pi, Constraint::any()))
                } else {
                    bail!(format_err!(
                        "can't resolve local dependency {} because it's \
                        parent package {} is not local.",
                        path.display(),
                        parent_pkg.name()
                    ))
                }
            }
            DepReq::Git { git, tag } => {
                let res = DirectRes::Git { repo: git, tag };
                let pi = PackageId::new(n, res.into());
                Ok((pi, Constraint::any()))
            }
        }
    }
}

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct Targets {
    pub lib: Option<LibTarget>,
    #[serde(default = "Vec::new")]
    pub bin: Vec<BinTarget>,
    #[serde(default = "Vec::new")]
    pub test: Vec<TestTarget>,
}

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LibTarget {
    #[serde(default = "default_lib_subpath")]
    pub path: SubPath,
    pub mods: Vec<String>,
    #[serde(default)]
    pub idris_opts: Vec<String>,
}

fn default_lib_subpath() -> SubPath {
    SubPath::from_path(Path::new("src")).unwrap()
}

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct BinTarget {
    pub name: String,
    #[serde(default = "default_bin_subpath")]
    pub path: SubPath,
    pub main: String,
    #[serde(default)]
    pub idris_opts: Vec<String>,
}

fn default_bin_subpath() -> SubPath {
    SubPath::from_path(Path::new("src")).unwrap()
}

/// A TestTarget is literally exactly the same as a BinTarget, with the only difference being
/// the difference in default path.
///
/// I know, code duplication sucks and is stupid, but what can ya do :v
#[serde(deny_unknown_fields)]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TestTarget {
    pub name: Option<String>,
    #[serde(default = "default_test_subpath")]
    pub path: SubPath,
    pub main: String,
    #[serde(default)]
    pub idris_opts: Vec<String>,
}

fn default_test_subpath() -> SubPath {
    SubPath::from_path(Path::new("tests")).unwrap()
}

impl From<TestTarget> for BinTarget {
    fn from(t: TestTarget) -> Self {
        let default_name = format!("test-{}", &t.main)
            .trim_end_matches(".idr")
            .trim_end_matches(".lidr")
            .replace("/", "_")
            .replace(".", "_");

        BinTarget {
            name: t.name.unwrap_or(default_name),
            path: t.path,
            main: t.main,
            idris_opts: t.idris_opts,
        }
    }
}

impl BinTarget {
    // A note on extensions:
    // - If the extension of the target_path is idr or empty, it will be treated as a Main file.
    // - If the extension of the target_path is anything else, that extension will be the function
    //   of the preceding part's module which will be treated as the main function.
    pub fn resolve_bin(&self, parent: &Path) -> Option<(PathBuf, PathBuf)> {
        let main_path: PathBuf = self.main.clone().into();
        // If the main path is a valid SubPath, we just use that.
        if let Ok(s) = SubPath::from_path(&main_path) {
            if parent.join(&s.0).with_extension("idr").exists() {
                let target_path = if s.0.extension().is_none() {
                    parent.join(&s.0).with_extension("idr")
                } else {
                    parent.join(&s.0)
                };
                let src_path = target_path.parent().unwrap();
                // This is the relative target path
                let target_path: PathBuf = target_path.file_name().unwrap().to_os_string().into();
                return Some((src_path.to_path_buf(), target_path));
            } else if parent.join(&s.0).with_extension("lidr").exists() {
                let target_path = if s.0.extension().is_none() {
                    parent.join(&s.0).with_extension("lidr")
                } else {
                    parent.join(&s.0)
                };
                let src_path = target_path.parent().unwrap();
                // This is the relative target path
                let target_path: PathBuf = target_path.file_name().unwrap().to_os_string().into();
                return Some((src_path.to_path_buf(), target_path));
            }
        }

        // Otherwise, we have to do more complicated logic.
        let src_path = parent.join(&self.path.0);
        let mut split = self.main.trim_matches('.').rsplitn(2, '.');
        let after = split.next().unwrap();
        let (after, before) = if after == "lidr" || after == "idr" {
            if let Some(before) = split.next() {
                let mut new_split = before.rsplitn(2, '.');
                let fpart = new_split.next().unwrap();
                (format!("{}.{}", fpart, after), new_split.next())
            } else {
                (after.to_owned(), None)
            }
        } else {
            (after.to_owned(), split.next())
        };

        if let Some(before) = before {
            let target_path: PathBuf = before.replace(".", "/").into();
            // If there is at least one dot in the name:
            if src_path
                .join(&target_path)
                .join(&after)
                .with_extension("idr")
                .exists()
            {
                // If a file corresponding to the whole module name exists, we use that.
                Some((src_path, target_path.join(after).with_extension("idr")))
            } else if src_path
                .join(&target_path)
                .join(&after)
                .with_extension("lidr")
                .exists()
            {
                // If a literate file corresponding to the whole module name exists, we use that.
                Some((src_path, target_path.join(after).with_extension("lidr")))
            } else if src_path.join(&target_path).with_extension("idr").exists() {
                // Otherwise, if a file corresponding to the module name minus the last
                // part exists, we assume that the last part refers to a function which
                // should be treated as the main function.
                Some((src_path, target_path.with_extension(after)))
            } else if src_path.join(&target_path).with_extension("lidr").exists() {
                // Same, but for literate file
                Some((src_path, target_path.with_extension(after)))
            } else {
                None
            }
        } else {
            let target_path: PathBuf = after.into();
            // Otherwise, if the name has no dots:
            if src_path.join(&target_path).with_extension("idr").exists() {
                Some((src_path, target_path.with_extension("idr")))
            } else if src_path.join(&target_path).with_extension("lidr").exists() {
                Some((src_path, target_path.with_extension("lidr")))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_valid() {
        let manifest = r#"
[package]
name = 'ring_ding/test'
version = '1.0.0'
authors = ['me']
license = 'MIT'
description = "The best package ever released"
homepage = "https://github.com/elba/elba"
repository = "https://github.com/elba/elba"
readme = "README.md"
keywords = ["package-manager", "packaging"]
exclude = ["*.blah"]

[dependencies]
'awesome/a' = '>= 1.0.0 < 2.0.0'
'cool/b' = { git = 'https://github.com/super/cool', tag = "v1.0.0" }
'great/c' = { path = 'here/right/now' }

[dev_dependencies]
'ayy/x' = '2.0'

[[targets.bin]]
name = 'bin1'
main = 'src/bin/Here'

[targets.lib]
path = "src/lib/"
mods = [
    "Control.Monad.Wow",
    "Control.Monad.Yeet",
    "RingDing.Test"
]
idris_opts = ["--warnpartial", "--warnreach"]
"#;

        assert!(Manifest::from_str(manifest).is_ok());
    }

    #[test]
    fn manifest_valid_no_targets() {
        let manifest = r#"
[package]
name = 'ring_ding/test'
version = '1.0.0'
authors = ['Me <y@boi.me>']
license = 'MIT'

[dependencies]
'awesome/a' = '>= 1.0.0 < 2.0.0'
'cool/b' = { git = 'https://github.com/super/cool', tag = "v1.0.0" }
'great/c' = { path = 'here/right/now' }

[dev_dependencies]
'ayy/x' = '2.0'
"#;

        assert!(Manifest::from_str(manifest).is_ok());
    }

    #[test]
    fn manifest_invalid_target_path() {
        let manifest = r#"
[package]
name = 'ring_ding/test'
version = '1.0.0'
description = "a cool package"
authors = ['me']
license = 'MIT'

[dependencies]
'awesome/a' = '>= 1.0.0 < 2.0.0'
'cool/b' = { git = 'https://github.com/super/cool', tag = "v1.0.0" }
'great/c' = { path = 'here/right/now' }

[dev_dependencies]
'ayy/x' = '2.0'

[targets.lib]
path = "../oops"
mods = [
    "Right.Here"
]
"#;

        assert!(Manifest::from_str(manifest).is_err());
    }
}
