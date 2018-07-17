// `err,rs` - because nothing ever goes right

use std::process::{ExitStatus, Output};

// TODO: More principled error handling.
//       Manifests and Constraints and whatever all have their own error enums. This kind enum just
//       provides context as to the error. So we'd have `ReqError::InvalidSigil` contextualized
//       with an `ErrorKind::InvalidLockfile`, etc.
// TODO: Intend to split this big enum into small structs error.
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

/// Process errors
#[derive(Debug, Fail)]
#[fail(display = "{}", desc)]
pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
}
