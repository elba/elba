//! Module `package/version` defines (syntax for) version and version range specifiers.
//!
//! ## NIH?
//! The semver crate's `Version` is fine. What's not fine is their `VersionReq.`
//!
//! The reason we're rolling our own instead of using something like the semver crate is that
//! the requirements for matic conflict with what semver provides. The vector-of-predicate
//! approach which semver provides is too flexible, making it harder to validate versions and
//! perform operations on them (check if one range is a subset of another, etc.). The semver crate
//! also provides some unnecessary operations.
//!
//! Instead, this module purposely restricts features and syntax to make code clearer, less bug-
//! prone, and more efficient.
//!
//! ## Functionality
//! Versions in matic take lots of good ideas from Cargo and Pub (Dart) versioning. We follow
//! Cargo's compatibility rules for 0.* and 0.0.* versions to allow for less-stable packages.
//! Additionally, we also follow Cargo's rules when sigils are omitted.
//! However, we purposely elide star notation since it's unnecessary; `0.* == 0`, `0.0.* == 0.0`.
//! To make parsing easier, `<` or `<=` must always precede `>` or `>=`, like with Pub. Nonsensical
//! requirements like `< 1 > 2` which are valid parses under semver get caught during parsing here.
//! In general, syntax is substantially stricter than in Cargo, and nonsensical constraints are
//! caught immediately when creating the constraint.

// Good ideas: https://pub.dartlang.org/packages/pub_semver

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use self::Interval::{Closed, Open, Unbounded};
use err::{Error, ErrorKind};
use nom::types::CompleteStr;
use semver::Version;
use std::{fmt, str::FromStr};

// TODO: Implement lul
/// A newtype wrapper for a `Version` which changes the ordering behavior such that the "greatest"
/// version is the one that a user would most likely prefer (the latest not-prerelease version)
pub struct OrderedVersion(Version);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interval {
    Open(Version),
    Closed(Version),
    Unbounded,
}

/// A range in which a version can fall into. Syntax for ranges mirrors that of something like
/// Pub or Cargo, but is substantially stricter. Whitespace is ignored, but clauses must be
/// separated by commas. Multiple clauses are only allowed when specifying less-than/greater-than
/// constraints. The following are all the valid cases the parser will accept (not including
/// variations in whitespace):
///
/// ```none
/// ~1
/// ~1.4
/// ~1.5.3
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    lower: Interval,
    upper: Interval,
}

impl Range {
    /// Creates a new `Range`.
    ///
    /// All `Range`s have to be valid, potentially true constraints. That is to say, the lower a If a nonsensical range is
    /// suggested, `None` is returned.
    pub fn new(lower: Interval, upper: Interval) -> Option<Range> {
        match (lower, upper) {
            (Open(lower), Open(upper)) => {
                if lower >= upper {
                    None
                } else {
                    Some(Range {
                        lower: Open(lower),
                        upper: Open(upper),
                    })
                }
            }
            (Open(lower), Closed(upper)) => {
                if lower >= upper {
                    None
                } else {
                    Some(Range {
                        lower: Open(lower),
                        upper: Closed(upper),
                    })
                }
            }
            (Closed(lower), Open(upper)) => {
                if lower >= upper {
                    None
                } else {
                    Some(Range {
                        lower: Closed(lower),
                        upper: Open(upper),
                    })
                }
            }
            (Closed(lower), Closed(upper)) => {
                if lower > upper {
                    None
                } else {
                    Some(Range {
                        lower: Closed(lower),
                        upper: Closed(upper),
                    })
                }
            }
            (lower, upper) => Some(Range { lower, upper }),
        }
    }

    /// Checks if a version satisfies this `Range`. When dealing with pre-release versions,
    /// pre-releases can only satisfy ranges if the range explicitly mentions a pre-release in either
    /// the upper or lower bound (or if it's unbounded in the upper direction)
    pub fn satisfied(&self, version: &Version) -> bool {
        let upper_pre_ok = match &self.upper {
            Open(u) => u.is_prerelease(),
            Closed(u) => u.is_prerelease(),
            Unbounded => true,
        };
        let lower_pre_ok = match &self.lower {
            Open(l) => l.is_prerelease(),
            Closed(l) => l.is_prerelease(),
            Unbounded => false,
        };
        let pre_ok = upper_pre_ok || lower_pre_ok;

        let satisfies_upper = match &self.upper {
            Open(u) => version < u,
            Closed(u) => version <= u,
            Unbounded => true,
        };
        let satisfies_lower = match &self.lower {
            Open(l) => version > l,
            Closed(l) => version >= l,
            Unbounded => true,
        };
        let satisfies_pre = pre_ok || !version.is_prerelease();

        satisfies_upper && satisfies_lower && satisfies_pre
    }
}

impl FromStr for Range {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Don't lose all error info
        // We throw away the original error because of 'static lifetime bs...
        let p = parse::range(CompleteStr(s)).map(|o| o.1).map_err(|_| ErrorKind::InvalidRange)?;

        Ok(p)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.lower, &self.upper) {
            (Open(l), Open(u))     => write!(f, "> {} < {}", l, u),
            (Open(l), Closed(u))   => write!(f, "> {} <= {}", l, u),
            (Open(l), Unbounded)   => write!(f, "> {}", l),
            (Closed(l), Open(u))   => write!(f, ">= {} < {}", l, u),
            (Closed(l), Closed(u)) => write!(f, ">= {} <= {}", l, u),
            (Closed(l), Unbounded) => write!(f, ">= {}", l),
            (Unbounded, Open(u))   => write!(f, "< {}", u),
            (Unbounded, Closed(u)) => write!(f, "< {}", u),
            (Unbounded, Unbounded) => write!(f, "any"),
        }
    }
}

impl Serialize for Range {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Range {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

mod parse {
    use super::*;
    use nom::{digit, types::CompleteStr};
    use semver::Version;
    use std::str::FromStr;

    fn increment_caret(v: Version, minor_specified: bool, patch_specified: bool) -> Version {
        let mut v = v;

        if v.major > 0 || (!minor_specified && !patch_specified) {
            v.increment_major();
        } else {
            if v.minor > 0 {
                v.increment_minor();
            } else {
                v.increment_patch();
            }
        }

        v
    }

    fn increment_tilde(v: Version, minor_specified: bool, patch_specified: bool) -> Version {
        let mut v = v;

        if !minor_specified && !patch_specified {
            v.increment_major();
        } else {
            v.increment_minor();
        }

        v
    }

    fn build_range(lower: Interval, upper: Interval) -> Option<Range> {
        if lower == Interval::Unbounded && upper == Interval::Unbounded {
            None
        } else {
            Range::new(lower, upper)
        }
    }

    named!(to_u64<CompleteStr, u64>,
        map_res!(
          ws!(digit),
          |s: CompleteStr| { FromStr::from_str(s.0) }
        )
    );

    named!(bare_version_major<CompleteStr, (Version, bool, bool)>, do_parse!(
        major: to_u64 >>
        ((Version::new(major, 0, 0), false, false))
    ));

    named!(bare_version_minor<CompleteStr, (Version, bool, bool)>, do_parse!(
        major: to_u64 >>
        tag_s!(".") >>
        minor: to_u64 >>
        ((Version::new(major, minor, 0), true, false))
    ));

    named!(bare_version_from_str<CompleteStr, (Version, bool, bool)>, do_parse!(
        vers: parse_to!(Version) >>
        (vers, true, true)
    ));

    named!(pub bare_version<CompleteStr, (Version, bool, bool)>,
        alt!(bare_version_from_str | bare_version_minor | bare_version_major)
    );

    // TODO: Unwrapping is Bad
    named!(range_caret<CompleteStr, Range>, ws!(do_parse!(
        opt!(tag_s!("^")) >>
        version: bare_version >>
        (Range::new(Interval::Closed(version.0.clone()), Interval::Open(increment_caret(version.0, version.1, version.2))).unwrap())
    )));

    named!(range_tilde<CompleteStr, Range>, ws!(do_parse!(
        tag_s!("~") >>
        version: bare_version >>
        (Range::new(Interval::Closed(version.0.clone()), Interval::Open(increment_tilde(version.0, version.1, version.2))).unwrap())
    )));

    named!(interval_lt<CompleteStr, Interval>, ws!(do_parse!(
        tag_s!("<") >>
        version: bare_version >>
        (Interval::Open(version.0))
    )));

    named!(interval_le<CompleteStr, Interval>, ws!(do_parse!(
        tag_s!("<=") >>
        version: bare_version >>
        (Interval::Closed(version.0))
    )));

    named!(interval_gt<CompleteStr, Interval>, ws!(do_parse!(
        tag_s!(">") >>
        version: bare_version >>
        (Interval::Open(version.0))
    )));

    named!(interval_ge<CompleteStr, Interval>, ws!(do_parse!(
        tag_s!(">=") >>
        version: bare_version >>
        (Interval::Closed(version.0))
    )));

    named!(range_interval<CompleteStr, Range>, ws!(do_parse!(
        lower: alt!(interval_gt | interval_ge | value!(Interval::Unbounded)) >>
        upper: alt!(interval_lt | interval_le | value!(Interval::Unbounded)) >>
        (build_range(lower, upper).unwrap())
    )));

    named!(range_any<CompleteStr, Range>, do_parse!(
        tag_s!("any") >>
        (Range::new(Interval::Unbounded, Interval::Unbounded).unwrap())
    ));

    named!(pub range<CompleteStr, Range>,
        alt!(range_caret | range_tilde | range_interval | range_any)
    );
}

#[cfg(test)]
mod tests {
    use super::parse::*;
    use super::*;
    use nom::types::CompleteStr;
    use semver::Version;

    macro_rules! new_range {
        ($a:tt... $b:tt) => {
            Range::new(
                Interval::Open(Version::parse($a).unwrap()),
                Interval::Open(Version::parse($b).unwrap()),
            )
        };
        ($a:tt = .. $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap()),
                Interval::Open(Version::parse($b).unwrap()),
            )
        };
        ($a:tt..= $b:tt) => {
            Range::new(
                Interval::Open(Version::parse($a).unwrap()),
                Interval::Closed(Version::parse($b).unwrap()),
            )
        };
        ($a:tt = . = $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap()),
                Interval::Closed(Version::parse($b).unwrap()),
            )
        };
    }

    #[test]
    fn test_parse_bare() {
        let vs: Vec<CompleteStr> = vec![
            "1",
            "1.0",
            "1.0.0",
            "1.0.0-alpha.1",
            "1.0.0+b1231231",
            "1.0.0-alpha.1+b1231231",
        ].into_iter()
            .map(CompleteStr)
            .collect();

        for v in vs {
            assert!(bare_version(v).is_ok());
        }
    }

    #[test]
    fn test_parse_range_caret() {
        let vs = vec!["1", "1.4", "1.4.3", "0", "0.2", "0.2.3", "0.0.2"]
            .into_iter()
            .map(CompleteStr)
            .map(|s| range(s).unwrap().1)
            .collect::<Vec<_>>();

        let ns = vec![
            new_range!("1.0.0" = .."2.0.0").unwrap(),
            new_range!("1.4.0" = .."2.0.0").unwrap(),
            new_range!("1.4.3" = .."2.0.0").unwrap(),
            new_range!("0.0.0" = .."1.0.0").unwrap(),
            new_range!("0.2.0" = .."0.3.0").unwrap(),
            new_range!("0.2.3" = .."0.3.0").unwrap(),
            new_range!("0.0.2" = .."0.0.3").unwrap(),
        ];

        assert_eq!(ns, vs);
    }

    #[test]
    fn test_parse_range_tilde() {
        let vs = vec!["~1", "~1.4", "~1.4.3", "~0", "~0.2", "~0.2.3", "~0.0.2"]
            .into_iter()
            .map(CompleteStr)
            .map(|s| range(s).unwrap().1)
            .collect::<Vec<_>>();

        let ns = vec![
            new_range!("1.0.0" = .."2.0.0").unwrap(),
            new_range!("1.4.0" = .."1.5.0").unwrap(),
            new_range!("1.4.3" = .."1.5.0").unwrap(),
            new_range!("0.0.0" = .."1.0.0").unwrap(),
            new_range!("0.2.0" = .."0.3.0").unwrap(),
            new_range!("0.2.3" = .."0.3.0").unwrap(),
            new_range!("0.0.2" = .."0.1.0").unwrap(),
        ];

        assert_eq!(ns, vs);
    }
}
