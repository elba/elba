//! Because nothing ever works correctly...

pub use failure::Error;

pub type Res<T> = Result<T, Error>;

// TODO: More principled error handling.
// This big enum shouldn't exist; instead, we should have individual structs and enums which
// implement Fail for each type of error: Index failures, Parsing failures, etc.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "Invalid lockfile.")]
    InvalidLockfile,
    #[fail(display = "Invalid manifest file.")]
    InvalidManifestFile,
    #[fail(display = "Invalid source url.")]
    InvalidSourceUrl,
    #[fail(display = "Invalid package id.")]
    InvalidPackageId,
    #[fail(display = "Invalid constraint.")]
    InvalidConstraint,
    #[fail(display = "Invalid index structure.")]
    InvalidIndex,
    #[fail(display = "Package doesn't exist in index.")]
    PackageNotFound,
    #[fail(display = "Conflict resolution failure.")]
    NoConflictRes,
    #[fail(display = "Package is missing manifest.")]
    MissingManifest,
    #[fail(display = "Could not download package.")]
    CannotDownload,
    #[fail(display = "Checksum error.")]
    Checksum,
    #[fail(display = "Resource is locked.")]
    Locked,
    #[doc(hidden)]
    #[fail(display = "This should be impossible")]
    __Nonexhaustive,
}
