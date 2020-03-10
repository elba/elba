use std::{convert::TryFrom, str::FromStr};

use failure::format_err;

use indexmap::IndexMap;

use semver::Version;

use serde::Deserialize;

use super::*;
use crate::package::manifest::{BinTarget, LibTarget, Manifest, PackageInfo, Targets, TestTarget};

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Ipkg {
    /// Name associated with a package.
    name: String,
    /// List of packages this package depends on.
    pkgs: Vec<String>,
    /// Brief description of the package.
    brief: Option<String>,
    /// Version string to associate with the package.
    version: Option<String>,
    /// Location of the README file.
    readme: Option<String>,
    /// Description of the licensing information.
    license: Option<String>,
    /// Author information.
    author: Option<String>,
    /// Maintainer information.
    maintainer: Option<String>,
    /// Website associated with the package.
    homepage: Option<String>,
    /// Location of the source files.
    sourceloc: Option<String>,
    /// Location of the project's bug tracker.
    bugtracker: Option<String>,
    /// External dependencies.
    libs: Vec<String>,
    /// Object files required by the package.
    objs: Vec<String>,
    /// Makefile used to build external code. Used as part of the FFI process.
    makefile: Option<String>,
    /// List of options to give the compiler.
    opts: Vec<String>,
    /// Source directory for Idris files.
    sourcedir: String,
    /// Modules provided by the package.
    modules: Vec<String>,
    /// If an executable in which module can the Main namespace and function be found.
    main: Option<String>,
    /// What to call the executable.
    executable: Option<String>,
    /// Lists of tests to execute against the package.
    tests: Vec<String>,
}

impl TryFrom<Ipkg> for Manifest {
    type Error = failure::Error;

    fn try_from(ipkg: Ipkg) -> Result<Self> {
        let package = PackageInfo {
            name: Name::new("legacy".to_string(), ipkg.name)?,
            version: Version::new(0, 0, 0),
            authors: [ipkg.author, ipkg.maintainer]
                .iter()
                .filter_map(|id| id.clone())
                .into_iter()
                .collect(),
            description: ipkg.brief,
            keywords: Vec::new(),
            homepage: ipkg.homepage,
            repository: ipkg.sourceloc,
            readme: ipkg.readme.map(|readme| readme.parse()).transpose()?,
            license: ipkg.license,
            exclude: None,
        };

        let mut idris_opts = Vec::new();
        for pkg in ipkg.pkgs {
            idris_opts.push("-p".to_owned());
            idris_opts.push(pkg);
        }
        idris_opts.extend(ipkg.opts);

        let lib_target = if ipkg.modules.is_empty() {
            None
        } else {
            Some(LibTarget {
                path: ipkg.sourcedir.parse()?,
                mods: ipkg.modules,
                idris_opts: idris_opts.clone(),
            })
        };

        let bin_target = if let Some(main) = ipkg.main {
            vec![BinTarget {
                name: ipkg
                    .executable
                    .unwrap_or(main.rsplit(".").next().unwrap_or("output").to_owned()),
                path: ipkg.sourcedir.parse()?,
                main: main.parse()?,
                idris_opts: idris_opts.clone(),
            }]
        } else {
            vec![]
        };

        let mut test_targets = Vec::new();
        for test in ipkg.tests {
            test_targets.push(TestTarget {
                name: None,
                path: ipkg.sourcedir.parse()?,
                main: test.parse()?,
                idris_opts: idris_opts.clone(),
            })
        }

        let mut scripts = IndexMap::new();
        if let Some(makefile) = ipkg.makefile {
            scripts.insert("prebuild".to_owned(), format!("make -f {}", makefile));
        }

        Ok(Manifest {
            package,
            dependencies: IndexMap::new(),
            dev_dependencies: IndexMap::new(),
            targets: Targets {
                lib: lib_target,
                bin: bin_target,
                test: test_targets,
            },
            workspace: IndexMap::new(),
            scripts,
        })
    }
}

impl TryFrom<Manifest> for Ipkg {
    type Error = failure::Error;

    fn try_from(manifest: Manifest) -> Result<Self> {
        if manifest.targets.lib.is_none() {
            bail!(format_err!("ipkg must have a lib target"))
        }
        for path in manifest
            .targets
            .bin
            .iter()
            .map(|bin_target| &bin_target.path)
            .chain(
                manifest
                    .targets
                    .test
                    .iter()
                    .map(|test_target| &test_target.path),
            )
        {
            if path != &manifest.targets.lib.as_ref().unwrap().path {
                bail!(format_err!(
                    "ipkg must have a unambigious sourcepath, while {} and {} are found",
                    path.0.display(),
                    manifest.targets.lib.unwrap().path.0.display(),
                ))
            }
        }

        Ok(Ipkg {
            name: manifest.package.name.name().to_owned(),
            pkgs: Vec::new(),
            brief: manifest.package.description.clone(),
            version: Some(manifest.package.version.to_string()),
            readme: manifest
                .package
                .readme
                .map(|path| path.0.to_str().unwrap().to_owned()),
            license: manifest.package.license.clone(),
            author: manifest.package.authors.get(0).cloned(),
            maintainer: manifest.package.authors.get(1).cloned(),
            homepage: manifest.package.homepage.clone(),
            sourceloc: manifest.package.repository.clone(),
            bugtracker: None,
            libs: Vec::new(),
            objs: Vec::new(),
            makefile: None,
            opts: manifest.targets.lib.as_ref().unwrap().idris_opts.clone(),
            sourcedir: manifest
                .targets
                .lib
                .as_ref()
                .unwrap()
                .path
                .0
                .to_str()
                .unwrap()
                .to_owned(),
            modules: manifest.targets.lib.unwrap().mods,
            main: manifest.targets.bin.get(0).map(|bin| bin.main.to_owned()),
            executable: manifest.targets.bin.get(0).map(|bin| bin.name.to_owned()),
            tests: manifest
                .targets
                .test
                .iter()
                .map(|test| test.main.to_owned())
                .collect(),
        })
    }
}

impl FromStr for Ipkg {
    type Err = failure::Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut ipkg = Ipkg::default();
        let (_, items) = parse::parse_items(input).map_err(|err| err.to_owned())?;
        for item in items {
            match item {
                parse::IpkgItem::PackageName(name) => ipkg.name = name,
                parse::IpkgItem::Vec(key, mut val) => match key.as_str() {
                    "pkgs" => ipkg.pkgs = val,
                    "brief" => ipkg.brief = val.pop(),
                    "version" => ipkg.version = val.pop(),
                    "readme" => ipkg.readme = val.pop(),
                    "license" => ipkg.license = val.pop(),
                    "author" => ipkg.author = val.pop(),
                    "maintainer" => ipkg.maintainer = val.pop(),
                    "homepage" => ipkg.homepage = val.pop(),
                    "sourceloc" => ipkg.sourceloc = val.pop(),
                    "bugtracker" => ipkg.bugtracker = val.pop(),
                    "libs" => ipkg.libs = val,
                    "objs" => ipkg.objs = val,
                    "makefile" => ipkg.makefile = val.pop(),
                    "opts" => ipkg.opts = val,
                    "sourcedir" => {
                        ipkg.sourcedir = val
                            .pop()
                            .ok_or_else(|| format_err!("ipkg must have `sourcedir`"))?
                    }
                    "modules" => ipkg.modules = val,
                    "main" => ipkg.main = val.pop(),
                    "executable" => ipkg.executable = val.pop(),
                    "tests" => ipkg.tests = val,
                    _ => (),
                },
            }
        }
        Ok(ipkg)
    }
}

mod parse {
    use nom::{
        branch::alt,
        bytes::complete::{is_not, tag, take, take_until, take_while1},
        character::complete::{multispace0, space0, space1},
        combinator::not,
        multi::{many0_count, many1, separated_list},
        sequence::{delimited, tuple},
        IResult,
    };

    #[derive(Debug, PartialEq, Eq)]
    pub enum IpkgItem {
        PackageName(String),
        Vec(String, Vec<String>),
    }

    pub fn parse_items(i: &str) -> IResult<&str, Vec<IpkgItem>> {
        let (i, items) = many1(alt((package_name, key_vec)))(i)?;
        let (i, _) = multispace0(i)?;
        let (i, _) = not(take(1usize))(i)?;
        Ok((i, items))
    }

    fn package_name(i: &str) -> IResult<&str, IpkgItem> {
        let (i, _) = many0_count(comment)(i)?;
        let (i, _) = multispace0(i)?;
        let (i, _) = tag("package")(i)?;
        let (i, _) = space1(i)?;
        let (i, name) = take_while1(|c: char| valid_name_char(c))(i)?;
        Ok((i, IpkgItem::PackageName(name.to_owned())))
    }

    fn key_vec(i: &str) -> IResult<&str, IpkgItem> {
        let (i, key) = key(i)?;
        let (i, vec) = vec(i)?;
        Ok((i, (IpkgItem::Vec(key.to_owned(), vec))))
    }

    fn key(i: &str) -> IResult<&str, String> {
        let (i, _) = many0_count(comment)(i)?;
        let (i, _) = multispace0(i)?;
        let (i, key) = take_while1(|c: char| valid_name_char(c))(i)?;
        let (i, _) = space0(i)?;
        let (i, _) = tag("=")(i)?;
        Ok((i, key.to_owned()))
    }

    fn value(i: &str) -> IResult<&str, String> {
        alt((value_quote, value_plain))(i)
    }

    fn value_quote(i: &str) -> IResult<&str, String> {
        let (i, _) = multispace0(i)?;
        let (i, val) = delimited(tag("\""), is_not("\""), tag("\""))(i)?;
        Ok((i, val.to_owned()))
    }

    fn value_plain(i: &str) -> IResult<&str, String> {
        let (i, _) = multispace0(i)?;
        let (i, val) = take_while1(|c: char| c == '.' || valid_name_char(c))(i)?;
        Ok((i, val.to_owned()))
    }

    fn vec(i: &str) -> IResult<&str, Vec<String>> {
        separated_list(tuple((multispace0, tag(","), multispace0)), value)(i)
    }

    fn comment(i: &str) -> IResult<&str, ()> {
        alt((comment_mutltiline, comment_sigle))(i)
    }

    fn comment_mutltiline(i: &str) -> IResult<&str, ()> {
        let (i, _) = multispace0(i)?;
        let (i, _) = delimited(tag("{-"), take_until("-}"), tag("-}"))(i)?;
        Ok((i, ()))
    }

    fn comment_sigle(i: &str) -> IResult<&str, ()> {
        let (i, _) = multispace0(i)?;
        let (i, _) = delimited(tag("--"), take_until("\n"), tag("\n"))(i)?;
        Ok((i, ()))
    }

    fn valid_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_' || c == '-'
    }

    #[test]
    fn test_parse() {
        let input = r#"
package idris-maths 

{- This
    is a
comment
-}
{- This is a comment too-}

    sourcedir= src
-- comment
opts="--quiet"  -- comment
modules =
NumOps
        , Test , Test.Test1

tests = Test.testDouble
    , Test.testTriple
"#;

        let (_, items) = parse_items(input).unwrap();
        let expected = vec![
            IpkgItem::PackageName("idris-maths".to_owned()),
            IpkgItem::Vec("sourcedir".to_owned(), vec!["src".to_owned()]),
            IpkgItem::Vec("opts".to_owned(), vec!["--quiet".to_owned()]),
            IpkgItem::Vec(
                "modules".to_owned(),
                vec![
                    "NumOps".to_owned(),
                    "Test".to_owned(),
                    "Test.Test1".to_owned(),
                ],
            ),
            IpkgItem::Vec(
                "tests".to_owned(),
                vec!["Test.testDouble".to_owned(), "Test.testTriple".to_owned()],
            ),
        ];
        assert_eq!(items, expected);
    }
}
