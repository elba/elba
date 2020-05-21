//! Because nothing ever works correctly...

pub use failure::Fail;

pub type Result<T> = std::result::Result<T, failure::Error>;

// TODO: More principled error handling.
// This big enum shouldn't exist; instead, we should have individual structs and enums which
// implement Fail for each type of error: Index failures, Parsing failures, etc.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum Error {
    #[fail(display = "invalid source resolution specifier")]
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
