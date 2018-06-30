use index::Indices;
use package::lockfile::Lockfile;

/// Retrieves the best packages using both the indices available and a lockfile.
/// By default, prioritizes using a lockfile.
#[derive(Clone, Debug)]
pub struct Retriever {
    indices: Indices,
    lockfile: Lockfile,
}
