//! Because nothing ever works correctly...

pub use failure::Error;

pub type Res<T> = Result<T, Error>;

// TODO: More principled error handling.
// This big enum shouldn't exist; instead, we should have individual structs and enums which
// implement Fail for each type of error: Index failures, Parsing failures, etc.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "invalid lockfile")]
    InvalidLockfile,
    #[fail(display = "invalid source url")]
    InvalidSourceUrl,
    #[fail(display = "invalid package id")]
    InvalidPackageId,
    #[fail(display = "invalid constraint")]
    InvalidConstraint,
    #[fail(display = "invalid index structure")]
    InvalidIndex,
    #[fail(display = "package doesn't exist in index")]
    PackageNotFound,
    #[fail(display = "conflict resolution failure")]
    NoConflictRes,
    #[fail(display = "package is missing manifest")]
    MissingManifest,
    #[fail(display = "could not download package")]
    CannotDownload,
    #[fail(display = "checksum error")]
    Checksum,
    #[fail(display = "resource is locked")]
    Locked,
    #[doc(hidden)]
    #[fail(display = "if you see this error, everything is wrong")]
    __Nonexhaustive,
}
