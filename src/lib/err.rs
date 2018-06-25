// `err,rs` - because nothing ever goes right

use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

pub type Res<T> = Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

// TODO: More principled error handling.
//       Manifests and Constraints and whatever all have their own error enums. This kind enum just
//       provides context as to the error. So we'd have `ReqError::InvalidSigil` contextualized
//       with an `ErrorKind::InvalidLockfile`, etc.
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
    #[fail(display = "Index is missing a valid `index.toml` file.")]
    IndexInvalidConfig,
    #[fail(display = "Invalid index structure.")]
    InvalidIndex,
    #[fail(display = "Package doesn't exist in index.")]
    NotInIndex,
    #[doc(hidden)]
    #[fail(display = "This should be impossible")]
    __Nonexhaustive,
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
