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

use self::Interval::{Closed, Open, Unbounded};
use err::{Error, ErrorKind};
use indexmap::IndexSet;
use itertools::Itertools;
use nom::types::CompleteStr;
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    cmp, fmt, hash::{Hash, Hasher}, str::FromStr,
};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum Relation {
    Superset,
    Subset,
    Overlapping,
    Disjoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Interval {
    Closed(Version),
    Open(Version),
    Unbounded,
}

impl Interval {
    /// Compares two `Interval`s and returns an ordering. Unfortunately we can't use the Ord trait
    /// beacuse of the extra parameter `lower`.
    pub fn cmp(&self, other: &Interval, lower: bool) -> cmp::Ordering {
        match (self, other) {
            (Interval::Unbounded, Interval::Unbounded) => cmp::Ordering::Equal,
            (Interval::Unbounded, _) => if lower {
                cmp::Ordering::Less
            } else {
                cmp::Ordering::Greater
            },
            (_, Interval::Unbounded) => if lower {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            },
            (Interval::Open(a), Interval::Open(b)) => a.cmp(&b),
            (Interval::Closed(a), Interval::Closed(b)) => a.cmp(&b),
            (Interval::Open(a), Interval::Closed(b)) => {
                if a == b {
                    if lower {
                        cmp::Ordering::Greater
                    } else {
                        cmp::Ordering::Less
                    }
                } else {
                    a.cmp(&b)
                }
            }
            (Interval::Closed(a), Interval::Open(b)) => {
                if a == b {
                    if lower {
                        cmp::Ordering::Less
                    } else {
                        cmp::Ordering::Greater
                    }
                } else {
                    a.cmp(&b)
                }
            }
        }
    }

    pub fn min<'a>(&'a self, other: &'a Interval, lower: bool) -> &'a Interval {
        if self.cmp(other, lower) == cmp::Ordering::Greater {
            other
        } else {
            self
        }
    }

    pub fn max<'a>(&'a self, other: &'a Interval, lower: bool) -> &'a Interval {
        if self.cmp(other, lower) == cmp::Ordering::Less {
            other
        } else {
            self
        }
    }

    pub fn flip(self) -> Interval {
        match self {
            Interval::Closed(v) => Interval::Open(v),
            Interval::Open(v) => Interval::Closed(v),
            Interval::Unbounded => Interval::Unbounded,
        }
    }
}

/// A continguous range in which a version can fall into. Syntax for ranges mirrors that of
/// Pub or Cargo. Ranges can accept caret and tilde syntax, as well as less-than/greater-than
/// specifications (just like Cargo). Like Pub, the `any` Range is completely unbounded on
/// both sides. Pre-release `Version`s can satisfy a `Range` iff the `Range`
/// mentions a pre-release `Version` on either bound, or if the `Range` is unbounded on the upper
/// side.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Range {
    lower: Interval,
    upper: Interval,
}

impl Range {
    /// Creates a new `Range`.
    ///
    /// All `Range`s have to be valid, potentially true constraints. If a nonsensical range is
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

    /// Creates the empty Range, which satisfies nothing.
    pub fn empty() -> Range {
        let lower = Interval::Open(Version::new(1, 0, 0));
        let upper = Interval::Open(Version::new(1, 0, 0));

        Range { lower, upper }
    }

    pub fn any() -> Range {
        let lower = Interval::Unbounded;
        let upper = Interval::Unbounded;

        Range { lower, upper }
    }

    pub fn upper(&self) -> &Interval {
        &self.upper
    }

    pub fn lower(&self) -> &Interval {
        &self.lower
    }

    pub fn take(self) -> (Interval, Interval) {
        (self.lower, self.upper)
    }

    /// Checks if a version is satisfied by this `Range`. When dealing with pre-release versions,
    /// pre-releases can only satisfy ranges if the range explicitly mentions a pre-release in either
    /// the upper or lower bound (or if it's unbounded in the upper direction)
    pub fn satisfies(&self, version: &Version) -> bool {
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

    /// Returns the intersection of two `Range`s, or `None` if the two `Range`s are disjoint.
    ///
    /// This function is a method of Range since we will never generate multiple disjoint `Range`s
    /// from an intersection operation.
    fn intersection(&self, other: &Range) -> Option<Range> {
        let lower = self.lower.max(&other.lower, true);
        let upper = self.upper.min(&other.upper, false);

        Range::new(lower.clone(), upper.clone())
    }
}

impl From<Version> for Range {
    fn from(v: Version) -> Range {
        let lower = Interval::Closed(v.clone());
        let upper = Interval::Closed(v);
        Range { lower, upper }
    }
}

impl FromStr for Range {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Don't lose all error info
        // We throw away the original error because of 'static lifetime bs...
        let p = parse::range(CompleteStr(s))
            .map(|o| o.1)
            .map_err(|_| ErrorKind::InvalidConstraint)?;

        Ok(p)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.lower, &self.upper) {
            (Open(l), Open(u)) => write!(f, "> {} < {}", l, u),
            (Open(l), Closed(u)) => write!(f, "> {} <= {}", l, u),
            (Open(l), Unbounded) => write!(f, "> {}", l),
            (Closed(l), Open(u)) => write!(f, ">= {} < {}", l, u),
            (Closed(l), Closed(u)) => write!(f, ">= {} <= {}", l, u),
            (Closed(l), Unbounded) => write!(f, ">= {}", l),
            (Unbounded, Open(u)) => write!(f, "< {}", u),
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

/// A set of `Range`s combines to make a `Constraint`. `Constraint`s are the union of multiple
/// `Range`s. Upon manual creation or updating of a `Constraint`, the `Constraint` will unify all
/// of its `Range`s such that all of the `Range`s are disjoint. Unification is eager: it's done
/// whenever the set is modified to keep the internal representation of the set unified at all
/// times (this is useful for converting the `Constraint` to a string, since the `Display` trait
/// doesn't allow mutating self).
///
/// Syntax-wise, a `Constraint` is just a list of comma-separated ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    set: IndexSet<Range>,
}

impl Constraint {
    /// Creates a new `Constraint` from a set of `Range`s.
    pub fn new(ranges: IndexSet<Range>) -> Constraint {
        let mut c = Constraint { set: ranges };
        c.unify();
        c
    }

    pub fn empty() -> Constraint {
        Constraint { set: indexset!() }
    }

    pub fn any() -> Constraint {
        Range::any().into()
    }

    /// Inserts a `Range` into the set.
    pub fn insert(&mut self, range: Range) {
        self.set.insert(range);
        self.unify();
    }

    /// Borrows the set of `Range`s from this struct, unifying it in the process.
    pub fn retrieve(&self) -> &IndexSet<Range> {
        &self.set
    }

    /// Takes the set of `Range`s from this struct, unifying it in the process.
    pub fn take(self) -> IndexSet<Range> {
        self.set
    }

    /// Unifies all of the ranges in the set such that all of the ranges are disjoint.
    pub fn unify(&mut self) {
        // Note: we take &mut self here because it's more convenient for using with other functions.
        // Turning it back into just self would just be turning sort_by into sorted_by and removing
        // the .cloned() call.
        self.set.sort_by(|a, b| a.lower().cmp(b.lower(), true));

        self.set = self
            .set
            .iter()
            .cloned()
            .coalesce(|a, b| {
                if a.upper().cmp(b.lower(), false) == cmp::Ordering::Greater {
                    let lower = a.take().0;
                    let upper = b.take().1;
                    let r = Range::new(lower, upper).unwrap();
                    Ok(r)
                } else if a.upper().cmp(b.lower(), false) == cmp::Ordering::Equal {
                    if let (Interval::Open(_), Interval::Open(_)) = (a.upper(), b.lower()) {
                        Err((a, b))
                    } else {
                        let lower = a.take().0;
                        let upper = b.take().1;
                        let r = Range::new(lower, upper).unwrap();
                        Ok(r)
                    }
                } else {
                    let (a2, b2) = (a.clone(), b.clone());
                    let (al, au) = a.take();
                    let (bl, bu) = b.take();
                    if let (Interval::Open(v), Interval::Closed(w)) = (au, bl) {
                        if v == w {
                            let r = Range::new(al, bu).unwrap();
                            Ok(r)
                        } else {
                            Err((a2, b2))
                        }
                    } else {
                        Err((a2, b2))
                    }
                }
            })
            .collect();
    }

    /// Checks if a `Version` is satisfied by this `Constraint`.
    pub fn satisfies(&self, v: &Version) -> bool {
        if self.set.is_empty() {
            return false;
        }

        for s in &self.set {
            if !s.satisfies(v) {
                return false;
            }
        }

        true
    }

    pub fn intersection(&self, other: &Constraint) -> Constraint {
        let mut set = IndexSet::new();

        for r in &self.set {
            for s in &other.set {
                if let Some(r) = r.intersection(&s) {
                    set.insert(r);
                }
            }
        }

        // We skip unification because we already know that the set will be unified.
        // The only time we might not be unified is during creation or arbitrary insertion.
        Constraint { set }
    }

    pub fn union(&self, other: &Constraint) -> Constraint {
        let mut set = self.set.clone();
        set.extend(other.set.clone());

        Constraint { set }
    }

    pub fn difference(&self, other: &Constraint) -> Constraint {
        let mut set = IndexSet::new();

        for r in &self.set {
            let mut r = r.clone();
            let mut g = true;
            for s in &other.set {
                if g {
                    match r.lower().cmp(s.lower(), true) {
                        cmp::Ordering::Greater => {
                            //------------------//
                            //         [=r=]    //
                            // [==s==]          //
                            //------------------//
                            //        OR        //
                            //------------------//
                            //         [=r=]    //
                            // [===s===]        //
                            //------------------//
                            //        OR        //
                            //------------------//
                            //         [=r=]    //
                            // [====s====]      //
                            //------------------//
                            //        OR        //
                            //------------------//
                            //         [=r=]    //
                            // [======s======]  //
                            //------------------//
                            match r.lower().cmp(s.upper(), false) {
                                cmp::Ordering::Greater => {
                                    // Situation 1
                                }
                                cmp::Ordering::Equal => {
                                    // Situation 2
                                    // If they're the same, the lower bound will always be open no matter
                                    // what
                                    let lower = if let Interval::Open(_) = s.upper() {
                                        s.upper().clone()
                                    } else {
                                        s.upper().clone().flip()
                                    };
                                    let upper = r.upper().clone();
                                    r = Range::new(lower, upper).unwrap();
                                }
                                cmp::Ordering::Less => {
                                    // Situation 3 & 4
                                    let lower = s.upper().clone().flip();
                                    let upper = r.upper().clone();
                                    // We have to do the if let because in Situation 4 there is no valid
                                    // Range
                                    if let Some(range) = Range::new(lower, upper) {
                                        r = range;
                                    } else {
                                        g = false;
                                    }
                                }
                            }
                        }
                        cmp::Ordering::Less => {
                            //------------------//
                            // [=r=]            //
                            //       [==s==]    //
                            //------------------//
                            //        OR        //
                            //------------------//
                            // [==r==]          //
                            //       [==s==]    //
                            //------------------//
                            //        OR        //
                            //------------------//
                            // [====r====]      //
                            //       [==s==]    //
                            //------------------//
                            //        OR        //
                            //------------------//
                            // [======r======]  //
                            //       [==s==]    //
                            //------------------//
                            // Situations 1-3
                            match r.upper().cmp(s.lower(), false) {
                                cmp::Ordering::Less => {
                                    // Situation 1
                                }
                                cmp::Ordering::Equal => {
                                    // Situation 2
                                    let lower = if let Interval::Open(_) = r.upper() {
                                        r.upper().clone()
                                    } else {
                                        r.upper().clone().flip()
                                    };
                                    let upper = s.lower().clone().flip();
                                    r = Range::new(lower, upper).unwrap();
                                }
                                cmp::Ordering::Greater => {
                                    // Situations 3 & 4
                                    if r.upper().cmp(s.upper(), false) != cmp::Ordering::Greater {
                                        // Situation 3
                                        let lower = r.lower().clone();
                                        let upper = s.lower().clone().flip();
                                        r = Range::new(lower, upper).unwrap();
                                    } else {
                                        // Situation 4
                                        let l1 = r.lower().clone();
                                        let u1 = s.lower().clone().flip();

                                        let l2 = s.upper().clone().flip();
                                        let u2 = r.upper().clone();

                                        // We can do this because we have a guarantee that all ranges
                                        // in a set are disjoint.
                                        set.insert(Range::new(l1, u1).unwrap());
                                        r = Range::new(l2, u2).unwrap();
                                    }
                                }
                            }
                        }
                        cmp::Ordering::Equal => {
                            let lower = s.upper().clone().flip();
                            let upper = r.upper().clone();

                            if let Some(range) = Range::new(lower, upper) {
                                r = range;
                            } else {
                                g = false;
                            }
                        }
                    }
                }
            }

            if g {
                set.insert(r);
            }
        }

        Constraint { set }
    }

    pub fn complement(&self) -> Constraint {
        let any: Constraint = Range::new(Interval::Unbounded, Interval::Unbounded)
            .unwrap()
            .into();
        any.difference(self)
    }

    pub fn relation(&self, other: &Constraint) -> Relation {
        let i = &self.intersection(other);
        if i == other {
            Relation::Superset
        } else if i == self {
            Relation::Subset
        } else if i.set.is_empty() {
            Relation::Disjoint
        } else {
            Relation::Overlapping
        }
    }
}

impl Default for Constraint {
    fn default() -> Self {
        Constraint::any()
    }
}

impl Hash for Constraint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for r in &self.set {
            r.hash(state);
        }
    }
}

impl From<Range> for Constraint {
    fn from(r: Range) -> Constraint {
        let mut set = IndexSet::new();
        set.insert(r);

        Constraint { set }
    }
}

impl From<Version> for Constraint {
    fn from(v: Version) -> Constraint {
        let r: Range = v.into();
        r.into()
    }
}

impl FromStr for Constraint {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Don't lose all error info
        // We throw away the original error because of 'static lifetime bs...
        let p = parse::constraint(CompleteStr(s))
            .map(|o| o.1)
            .map_err(|_| ErrorKind::InvalidConstraint)?;

        Ok(p)
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.set.iter().map(|r| r.to_string()).join(", "))
    }
}

impl Serialize for Constraint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Constraint {
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

    named!(pub constraint<CompleteStr, Constraint>,
        alt!(map!(separated_list!(tag!(","), range), |v| { Constraint::new(v.into_iter().collect()) }) | value!(Constraint::default()))
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
            ).unwrap()
        };
        ($a:tt ~ .. $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap()),
                Interval::Open(Version::parse($b).unwrap()),
            ).unwrap()
        };
        ($a:tt.. ~ $b:tt) => {
            Range::new(
                Interval::Open(Version::parse($a).unwrap()),
                Interval::Closed(Version::parse($b).unwrap()),
            ).unwrap()
        };
        ($a:tt ~ . ~ $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap()),
                Interval::Closed(Version::parse($b).unwrap()),
            ).unwrap()
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
            new_range!("1.0.0" ~.. "2.0.0"),
            new_range!("1.4.0" ~.. "2.0.0"),
            new_range!("1.4.3" ~.. "2.0.0"),
            new_range!("0.0.0" ~.. "1.0.0"),
            new_range!("0.2.0" ~.. "0.3.0"),
            new_range!("0.2.3" ~.. "0.3.0"),
            new_range!("0.0.2" ~.. "0.0.3"),
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
            new_range!("1.0.0" ~.. "2.0.0"),
            new_range!("1.4.0" ~.. "1.5.0"),
            new_range!("1.4.3" ~.. "1.5.0"),
            new_range!("0.0.0" ~.. "1.0.0"),
            new_range!("0.2.0" ~.. "0.3.0"),
            new_range!("0.2.3" ~.. "0.3.0"),
            new_range!("0.0.2" ~.. "0.1.0"),
        ];

        assert_eq!(ns, vs);
    }

    #[test]
    fn test_constraint_complement_symmetry() {
        let rs = indexset!(
            new_range!("1.0.0" ~.. "2.0.0"),
            new_range!("1.5.0" ~.~ "2.6.1")
        );
        let c: Constraint = Constraint::new(rs);

        assert_eq!(c, c.complement().complement());
    }

    #[test]
    fn test_constraint_subset() {
        let a = Constraint::new(indexset!(
            new_range!("1.0.0" ..~ "1.3.2"),
            new_range!("5.5.7" ~.~ "5.6.2-alpha.2")
        ));
        let b = Constraint::new(indexset!(
            new_range!("1.0.0" ~.~ "1.5.2"),
            new_range!("5.3.2" ~.~ "5.7.7")
        ));

        assert_eq!(Relation::Subset, a.relation(&b));
        assert_eq!(Relation::Superset, b.relation(&a));
    }

    #[test]
    fn test_version_single() {
        let v = Version::new(1, 4, 2);
        let v2 = Version::new(1, 4, 3);
        let v3 = Version::new(1, 4, 1);
        let c: Constraint = v.clone().into();

        assert!(c.satisfies(&v));
        assert!(!c.satisfies(&v2));
        assert!(!c.satisfies(&v3));
    }
}
