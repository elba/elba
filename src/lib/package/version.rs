//! Defines (syntax for) version and version range specifiers.
//!
//! ## NIH?
//! The semver crate's `Version` is fine. What's not fine is their `VersionReq.`
//!
//! The reason we're rolling our own instead of using something like the semver crate is that
//! the requirements for elba conflict with what semver provides. The vector-of-predicate
//! approach which semver provides is too flexible, making it harder to validate versions and
//! perform operations on them (check if one range is a subset of another, etc.). The semver crate
//! also provides some unnecessary operations.
//!
//! Instead, this module adds features in some places and removes others for flexibility where it
//! matters for elba.
//!
//! ## Functionality
//! Versions in elba take lots of good ideas from Cargo and Pub (Dart) versioning. We follow
//! Cargo's compatibility rules for 0.* and 0.0.* versions to allow for less-stable packages.
//! Additionally, we also follow Cargo's rules when sigils are omitted.
//! However, we purposely elide star notation since it's unnecessary; `0.* == 0`, `0.0.* == 0.0`.
//! To make parsing easier, `<` or `<=` must always precede `>` or `>=`, like with Pub. Nonsensical
//! requirements like `< 1 > 2` which are valid parses under semver get caught during parsing here.
//! In general, syntax is substantially stricter than in Cargo, and nonsensical constraints are
//! caught immediately when creating the constraint.

// Good ideas: https://pub.dartlang.org/packages/pub_semver

use self::Interval::{Closed, Open, Unbounded};
use failure::{format_err, Error};
use indexmap::{indexset, IndexSet};
use itertools::Itertools;
use nom::types::CompleteStr;
use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp, fmt, str::FromStr};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum Relation {
    Superset,
    Subset,
    Overlapping,
    Disjoint,
    Equal,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Interval {
    Closed(Version, bool),
    Open(Version, bool),
    Unbounded,
}

impl Interval {
    /// Compares two `Interval`s and returns an ordering. Unfortunately we can't use the Ord trait
    /// beacuse of the extra parameter `lower`.
    pub fn cmp(&self, other: &Interval, lower: bool) -> cmp::Ordering {
        match (self, other) {
            (Interval::Unbounded, Interval::Unbounded) => cmp::Ordering::Equal,
            (Interval::Unbounded, _) => {
                if lower {
                    cmp::Ordering::Less
                } else {
                    cmp::Ordering::Greater
                }
            }
            (_, Interval::Unbounded) => {
                if lower {
                    cmp::Ordering::Greater
                } else {
                    cmp::Ordering::Less
                }
            }
            (Interval::Open(a, ap), Interval::Open(b, bp)) => {
                let c = a.cmp(&b);
                if c == cmp::Ordering::Equal {
                    if lower {
                        // >! == >
                        cmp::Ordering::Equal
                    } else {
                        // <! > <
                        ap.cmp(&bp)
                    }
                } else {
                    c
                }
            }
            (Interval::Closed(a, ap), Interval::Closed(b, bp)) => {
                let c = a.cmp(&b);
                if c == cmp::Ordering::Equal {
                    if lower {
                        ap.cmp(&bp).reverse()
                    } else {
                        ap.cmp(&bp)
                    }
                } else {
                    c
                }
            }
            (Interval::Open(a, _), Interval::Closed(b, _)) => {
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
            (Interval::Closed(a, _), Interval::Open(b, _)) => {
                if a == b {
                    if lower {
                        // The pre_ok doesn't matter, cause >! == >
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
            Interval::Closed(v, pre_ok) => Interval::Open(v, !pre_ok),
            Interval::Open(v, pre_ok) => Interval::Closed(v, !pre_ok),
            Interval::Unbounded => Interval::Unbounded,
        }
    }

    pub fn pre_ok(&self) -> bool {
        match self {
            Interval::Closed(_, pre_ok) => *pre_ok,
            Interval::Open(_, pre_ok) => *pre_ok,
            Interval::Unbounded => true,
        }
    }

    pub fn show(&self, lower: bool) -> String {
        match &self {
            Interval::Unbounded => "".to_string(),
            Interval::Closed(v, p) => {
                if lower {
                    if !v.is_prerelease() && *p {
                        format!(">=!{}", v)
                    } else {
                        format!(">={}", v)
                    }
                } else {
                    // <=! and <= are equivalent, so there's no point confusing people
                    // with separate sigils
                    format!("<={}", v)
                }
            }
            Interval::Open(v, p) => {
                if lower {
                    // Same thing with >! and >.
                    format!(">{}", v)
                } else if !v.is_prerelease() && *p {
                    format!("<!{}", v)
                } else {
                    format!("<{}", v)
                }
            }
        }
    }
}

/// A continguous range in which a version can fall into. Syntax for ranges mirrors that of
/// Pub or Cargo. Ranges can accept caret and tilde syntax, as well as less-than/greater-than
/// specifications (just like Cargo). Like Pub, the `any` Range is completely unbounded on
/// both sides. Pre-release `Version`s can satisfy a `Range` iff the `Range`
/// mentions a pre-release `Version` on either bound, or if the `Range` is unbounded on the upper
/// side. Additionally, if a greater-than and/or less-than `Range` also has a `!` after the
/// inequality symbol, the Range will include pre-release versions. `>=! 1.0.0` accepts all
/// pre-releases of 1.0.0, along with the greater versions. `<! 2.0.0` includes pre-releases of
/// 2.0.0. >! and > mean the same thing, as do <=! and <=.
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
            (Interval::Unbounded, b) => Some(Range {
                lower: Interval::Unbounded,
                upper: b,
            }),
            (a, Interval::Unbounded) => Some(Range {
                lower: a,
                upper: Interval::Unbounded,
            }),
            (Interval::Open(a, ap), Interval::Closed(b, bp)) => {
                if a == b {
                    None
                } else {
                    let (a, b) = (Interval::Open(a, ap), Interval::Closed(b, bp));
                    if a.cmp(&b, true) != cmp::Ordering::Greater {
                        Some(Range { lower: a, upper: b })
                    } else {
                        None
                    }
                }
            }
            (Interval::Closed(a, ap), Interval::Open(b, bp)) => {
                if a == b && !(ap && bp) {
                    None
                } else {
                    let (a, b) = (Interval::Closed(a, ap), Interval::Open(b, bp));
                    if a.cmp(&b, true) != cmp::Ordering::Greater {
                        Some(Range { lower: a, upper: b })
                    } else {
                        None
                    }
                }
            }
            (Interval::Open(a, ap), Interval::Open(b, bp)) => {
                if a == b {
                    None
                } else {
                    let (a, b) = (Interval::Open(a, ap), Interval::Open(b, bp));
                    if a.cmp(&b, true) != cmp::Ordering::Greater {
                        Some(Range { lower: a, upper: b })
                    } else {
                        None
                    }
                }
            }
            (a, b) => {
                if a.cmp(&b, true) != cmp::Ordering::Greater {
                    Some(Range { lower: a, upper: b })
                } else {
                    None
                }
            }
        }
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

    /// Checks if a version is satisfied by this `Range`.
    pub fn satisfies(&self, version: &Version) -> bool {
        // For an upper range, a pre-release will satisfy the upper range if the interval is Open
        // and it either is a prerelease or always accepts prereleases, or if the Interval is
        // Closed or Unbounded. (`<= 2.0.0` includes 2.0.0-alpha, etc. <= is the same as <=!, and
        // as we'll see later, >! is the same as >)
        let upper_pre_ok = match &self.upper {
            Open(u, p) => u.is_prerelease() || *p,
            _ => true,
        };

        let satisfies_upper = match &self.upper {
            Open(u, _) => version < u,
            Closed(u, _) => version <= u,
            Unbounded => true,
        };
        let satisfies_lower = match &self.lower {
            Open(l, false) => version > l,
            Closed(l, false) => version >= l,
            // >! is the same as >, since > means "ignoring this release, anything above"; the
            // prereleases would've been ignored either way.
            Open(l, true) => version > l,
            // >=! is not the same as >=. >= ignores pre-releases of the version. >=! doesn't.
            Closed(l, true) => {
                (version.major, version.minor, version.patch) >= (l.major, l.minor, l.patch)
            }
            Unbounded => true,
        };

        satisfies_lower && satisfies_upper && (!version.is_prerelease() || upper_pre_ok)
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
        let lower = Interval::Closed(v.clone(), false);
        let upper = Interval::Closed(v, false);
        Range { lower, upper }
    }
}

impl FromStr for Range {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let p = parse::range(CompleteStr(s))
            .map(|o| o.1)
            .map_err(|_| format_err!("invalid constraint syntax"))?
            .ok_or_else(|| format_err!("invalid constraint"))?;

        Ok(p)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.lower, &self.upper) {
            (Unbounded, Unbounded) => write!(f, "any"),
            (Unbounded, b) => write!(f, "{}", b.show(false)),
            (a, Unbounded) => write!(f, "{}", a.show(true)),
            (a, b) => write!(f, "{} {}", a.show(true), b.show(false)),
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
#[derive(Clone, PartialEq, Eq)]
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
                    if let (Interval::Open(_, _), Interval::Open(_, _)) = (a.upper(), b.lower()) {
                        return Err((a, b));
                    }
                    let lower = a.take().0;
                    let upper = b.take().1;
                    let r = Range::new(lower, upper).unwrap();
                    Ok(r)
                } else {
                    let (a2, b2) = (a.clone(), b.clone());
                    let (al, au) = a.take();
                    let (bl, bu) = b.take();
                    if let (Interval::Open(v, _), Interval::Closed(w, _)) = (au, bl) {
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

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Checks if a `Version` is satisfied by this `Constraint`.
    pub fn satisfies(&self, v: &Version) -> bool {
        if self.set.is_empty() {
            return false;
        }

        for s in &self.set {
            if s.satisfies(v) {
                return true;
            }
        }

        false
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
                                // Do nothing
                            }
                            cmp::Ordering::Equal => {
                                // Situation 2
                                // If they're the same, the lower bound will always be open no matter
                                // what
                                let lower = if let Interval::Open(_, _) = s.upper() {
                                    s.upper().clone()
                                } else {
                                    s.upper().clone().flip()
                                };
                                let upper = r.upper().clone();
                                r = Range::new(lower, upper).unwrap();
                            }
                            cmp::Ordering::Less => {
                                // Situation 3 & 4
                                // Special-case for Unbounded because that screws with things
                                if s.upper() == &Interval::Unbounded {
                                    g = false;
                                    break;
                                }
                                let lower = s.upper().clone().flip();
                                let upper = r.upper().clone();
                                // We have to do the if let because in Situation 4 there is no valid
                                // Range
                                if let Some(range) = Range::new(lower, upper) {
                                    r = range;
                                } else {
                                    g = false;
                                    break;
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
                                let lower = r.lower().clone();
                                let upper = match (r.upper(), s.lower()) {
                                    (Interval::Closed(a, ap), _) => Interval::Open(a.clone(), *ap),
                                    (Interval::Open(_, _), _) => r.upper().clone(),
                                    (_, _) => unreachable!(),
                                };
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
                        if s.upper() == r.upper() {
                            g = false;
                            break;
                        }

                        // Again, special-casing the Unbounded case.
                        if s.upper() == &Interval::Unbounded {
                            g = false;
                            break;
                        }

                        let lower = s.upper().clone().flip();
                        let upper = r.upper().clone();

                        if let Some(range) = Range::new(lower, upper) {
                            r = range;
                        } else {
                            g = false;
                            break;
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
        if i == other && i == self {
            Relation::Equal
        } else if i == other {
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
        let p = parse::constraint(CompleteStr(s))
            .map(|o| o.1)
            .map_err(|_| format_err!("invalid constraint syntax"))?
            .ok_or_else(|| format_err!("invalid constraint"))?;

        Ok(p)
    }
}

impl fmt::Debug for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Constraint({})",
            self.set.iter().map(|r| r.to_string()).join(", ")
        )
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
    use nom::{
        alt, alt_complete, call, complete, digit, do_parse, do_parse_sep, error_position, map,
        map_res, named, opt, sep, separated_list, tag, tag_s, take_while1, take_while1_s,
        types::CompleteStr, wrap_sep, ws,
    };
    use semver::Version;
    use std::str::FromStr;

    fn increment_caret(v: Version, minor_specified: bool, patch_specified: bool) -> Version {
        let mut v = v;

        if v.major > 0 || (!minor_specified && !patch_specified) {
            v.increment_major();
        } else if v.minor > 0 {
            v.increment_minor();
        } else {
            v.increment_patch();
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

    fn version_char(chr: char) -> bool {
        chr == '.' || chr.is_digit(10)
    }

    fn build_range(
        lower: Option<(bool, Version, bool)>,
        upper: Option<(bool, Version, bool)>,
    ) -> Option<Range> {
        let lower = if let Some(lower) = lower {
            if lower.0 {
                Interval::Closed(lower.1, lower.2)
            } else {
                Interval::Open(lower.1, lower.2)
            }
        } else {
            Interval::Unbounded
        };

        let upper = if let Some(upper) = upper {
            if upper.0 {
                Interval::Closed(upper.1, upper.2)
            } else {
                Interval::Open(upper.1, upper.2)
            }
        } else {
            Interval::Unbounded
        };

        if lower == Interval::Unbounded && upper == Interval::Unbounded {
            return None;
        }

        Range::new(lower, upper)
    }

    named!(to_u64<CompleteStr, u64>,
        map_res!(digit, |s: CompleteStr| FromStr::from_str(s.0))
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

    named!(bare_version_from_str<CompleteStr, (Version, bool, bool)>,
        map_res!(take_while1_s!(version_char), |s: CompleteStr| Version::parse(s.0).map(|v| (v, true, true)))
    );

    named!(pub bare_version<CompleteStr, (Version, bool, bool)>,
        alt!(bare_version_from_str | bare_version_minor | bare_version_major)
    );

    named!(range_caret<CompleteStr, Option<Range>>, ws!(do_parse!(
        opt!(tag_s!("^")) >>
        version: bare_version >>
        (Range::new(Interval::Closed(version.0.clone(), false), Interval::Open(increment_caret(version.0, version.1, version.2), false)))
    )));

    named!(range_tilde<CompleteStr, Option<Range>>, ws!(do_parse!(
        tag_s!("~") >>
        version: bare_version >>
        (Range::new(Interval::Closed(version.0.clone(), false), Interval::Open(increment_tilde(version.0, version.1, version.2), false)))
    )));

    // Note! We allow bangs where they don't change anything (>=! and <!)
    named!(range_interval<CompleteStr, Option<Range>>, ws!(do_parse!(
        lower: opt!(ws!(do_parse!(
            tag_s!(">") >>
            a: opt!(tag_s!("=")) >>
            inf: opt!(tag_s!("!")) >>
            version: bare_version >>
            (a.is_some(), version.0.clone(), inf.is_some() && !version.0.is_prerelease())
        ))) >>
        upper: opt!(ws!(do_parse!(
            tag_s!("<") >>
            a: opt!(tag_s!("=")) >>
            inf: opt!(tag_s!("!")) >>
            version: bare_version >>
            (a.is_some(), version.0.clone(), inf.is_some() && !version.0.is_prerelease())
        ))) >>
        (build_range(lower, upper))
    )));

    named!(range_any<CompleteStr, Option<Range>>, do_parse!(
        tag_s!("any") >>
        (Range::new(Interval::Unbounded, Interval::Unbounded))
    ));

    named!(pub range<CompleteStr, Option<Range>>,
        alt_complete!(range_caret | range_tilde | range_any | range_interval)
    );

    named!(pub constraint<CompleteStr, Option<Constraint>>,
        map!(separated_list!(tag!(","), range), |v| if v.contains(&None) { None } else { Some(Constraint::new(v.into_iter().map(|x| x.unwrap()).collect())) })
    );
}

#[cfg(test)]
mod tests {
    use super::{parse::*, *};
    use semver::Version;

    macro_rules! new_range {
        ($a:tt... $b:tt) => {
            Range::new(
                Interval::Open(Version::parse($a).unwrap()),
                Interval::Open(Version::parse($b).unwrap()),
            )
            .unwrap()
        };
        ($a:tt ~ .. $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap(), false),
                Interval::Open(Version::parse($b).unwrap(), false),
            )
            .unwrap()
        };
        ($a:tt.. ~ $b:tt) => {
            Range::new(
                Interval::Open(Version::parse($a).unwrap(), false),
                Interval::Closed(Version::parse($b).unwrap(), false),
            )
            .unwrap()
        };
        ($a:tt ~ . ~ $b:tt) => {
            Range::new(
                Interval::Closed(Version::parse($a).unwrap(), false),
                Interval::Closed(Version::parse($b).unwrap(), false),
            )
            .unwrap()
        };
    }

    #[test]
    fn version_parse() {
        let vs = vec![
            "1",
            "1.0",
            "1.0.0",
            "1.0.0-alpha.1",
            "1.0.0+b1231231",
            "1.0.0-alpha.1+b1231231",
        ]
        .into_iter()
        .map(|v| CompleteStr(v))
        .collect::<Vec<_>>();

        for v in vs {
            assert!(bare_version(v).is_ok());
        }
    }

    #[test]
    fn range_parse_caret() {
        let vs = vec!["1", "1.4", "1.4.3", "0", "0.2", "0.2.3", "0.0.2"]
            .into_iter()
            .map(|s| Range::from_str(s).unwrap())
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
    fn range_parse_tilde() {
        let vs = vec!["~1", "~1.4", "~1.4.3", "~0", "~0.2", "~0.2.3", "~0.0.2"]
            .into_iter()
            .map(|s| Range::from_str(s).unwrap())
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
    fn range_parse_manual() {
        let vs = vec![">= 1.0.0 < 2.0.0", ">= 1.0.0", "< 2.0.0"]
            .into_iter()
            .map(|s| Range::from_str(s).unwrap())
            .collect::<Vec<_>>();

        let ns = vec![
            new_range!("1.0.0" ~.. "2.0.0"),
            Range::new(
                Interval::Closed(Version::parse("1.0.0").unwrap(), false),
                Interval::Unbounded,
            )
            .unwrap(),
            Range::new(
                Interval::Unbounded,
                Interval::Open(Version::parse("2.0.0").unwrap(), false),
            )
            .unwrap(),
        ];

        assert_eq!(ns, vs);
    }

    #[test]
    fn range_pre_ok() {
        let r = Range::from_str(">=! 1.0.0 <! 2.0.0").unwrap();
        let vs = vec!["1.0.0-alpha.1", "1.0.0", "1.4.2", "1.6.3", "2.0.0-alpha.1"];
        for v in vs {
            let v = Version::parse(v).unwrap();
            assert!(r.satisfies(&v));
        }
    }

    #[test]
    fn constraint_relation() {
        let rs = indexset!(Range::from_str("1").unwrap(),);
        let c = Constraint::new(rs);

        let c2 = c.complement();

        assert_eq!(Relation::Disjoint, c.relation(&c2));
    }

    #[test]
    fn constraint_relation_single() {
        let rs = indexset!(
            Range::from_str("<! 1.0.0").unwrap(), // pre-releases of 1.0.0 ok
            Range::from_str(">! 1.0.0").unwrap(), // same as > 1.0.0
        );
        let c = Constraint::new(rs);

        let r2 = indexset!(Range::from_str(">= 1.0.0 <= 1.0.0").unwrap(),);
        let c2 = Constraint::new(r2);

        assert_eq!(c.relation(&c2), Relation::Disjoint);
    }

    #[test]
    fn constraint_relation_opposite() {
        let c1 = Constraint::from_str(">! 2.0.0 < 3.0.0").unwrap();
        let c2 = Constraint::from_str("<! 2.0.0, >=! 3.0.0").unwrap();

        assert_eq!(Relation::Disjoint, c1.relation(&c2));
    }

    #[test]
    fn constraint_complement() {
        let rs = indexset!(
            new_range!("1.0.0" ~.. "2.0.0"),
            new_range!("2.5.3-beta.1" ~.~ "2.7.8")
        );
        let c = Constraint::new(rs);
        let a = c.complement();

        let vs = vec![
            "1.0.0-alpha.1",
            "1.0.0",
            "1.4.2",
            "1.6.3",
            "2.0.0-alpha.1",
            "2.5.3-alpha.1",
            "2.5.3-zeta.1",
            "2.7.8-beta.3",
        ];

        for v in vs {
            let v = Version::parse(v).unwrap();
            assert_eq!(c.satisfies(&v), !a.satisfies(&v));
        }
    }

    #[test]
    fn constraint_complement_symmetry() {
        let rs = indexset!(
            new_range!("1.0.0" ~.. "2.0.0"),
            new_range!("1.5.0" ~.~ "2.6.1")
        );
        let c = Constraint::new(rs);

        assert_eq!(c, c.complement().complement());
    }

    #[test]
    fn constraint_complement_correct() {
        let c1 = Constraint::from_str("<! 2.0.0, >=! 3.0.0").unwrap();
        let c2 = Constraint::from_str(">= 2.0.0 < 3.0.0").unwrap();

        assert_eq!(c2, c1.complement());
    }

    #[test]
    fn constraint_complement_single() {
        let rs = indexset!(
            Range::from_str("<! 1.0.0").unwrap(), // pre-releases of 1.0.0 ok
            Range::from_str(">! 1.0.0").unwrap(), // same as > 1.0.0
        );
        let c = Constraint::new(rs);

        let r2 = indexset!(Range::from_str(">= 1.0.0 <= 1.0.0").unwrap(),);
        let c2 = Constraint::new(r2);

        assert_eq!(c.complement(), c2);
    }

    #[test]
    fn constraint_subset() {
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
    fn version_single() {
        let v = Version::new(1, 4, 2);
        let v2 = Version::new(1, 4, 3);
        let v3 = Version::new(1, 4, 1);
        let c: Constraint = v.clone().into();

        assert!(c.satisfies(&v));
        assert!(!c.satisfies(&v2));
        assert!(!c.satisfies(&v3));
    }

    #[test]
    fn constraint_difference() {
        let c1 = Constraint::from_str("<! 2.0.0").unwrap();
        let c2 = Constraint::from_str(">! 2.0.0 < 3.0.0").unwrap();

        let _ = c1.difference(&c2);

        // just check that this doesn't unwrap on None.
    }

    #[test]
    fn constraint_difference_any() {
        let c1 = Constraint::from_str(">= 2.0.0 < 3.0.0").unwrap();
        let c2 = Constraint::from_str("any").unwrap();
        let ce = Constraint::empty();

        assert_eq!(ce, c1.difference(&c2));
    }

    #[test]
    fn constraint_difference_any_union() {
        let c1 = Constraint::from_str("<! 1.1.0, >= 2.0.0").unwrap();
        let c2 = Constraint::from_str("any").unwrap();
        let ce = Constraint::empty();

        assert_eq!(ce, c1.difference(&c2));
    }
}
