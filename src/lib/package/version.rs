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

use semver::Version;

/// A newtype wrapper for a `Version` which changes the ordering behavior such that the "greatest"
/// version is the one that a user would most likely prefer (the latest not-prerelease version)
pub struct OrderedVersion(Version);

pub enum Interval {
    Open(u16),
    Closed(u16),
    Unbounded,
}

// TODO: Document syntax (ignore whitespace)
/// Defines a requirment for a version.
pub struct VersionReq {
    lower: Version,
    upper: Version,
}
