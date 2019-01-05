//! Because nothing ever works correctly...

pub use failure::Error;
use failure_derive::Fail;

pub type Res<T> = Result<T, Error>;

// TODO: More principled error handling.
// This big enum shouldn't exist; instead, we should have individual structs and enums which
// implement Fail for each type of error: Index failures, Parsing failures, etc.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "invalid source url")]
    InvalidSourceUrl,
    #[fail(display = "package doesn't exist in index")]
    PackageNotFound,
    #[fail(display = "conflict resolution failure")]
    NoConflictRes,
    #[fail(display = "could not download package")]
    CannotDownload,
    #[doc(hidden)]
    #[fail(display = "if you see this error, everything is wrong")]
    __Nonexhaustive,
}
