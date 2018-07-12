//! Caching packages which have been downloaded before.
//! 
//! ## Background: Previous design
//! Previous designs for `elba` indices alluded to a feature where the local package cache would
//! be formatted as an index itself, with its entries pointing to locations on disk where
//! downloaded packages (whether as a result of DirectRes deps by root or through Indices)
//! are located. However, this relied on the concept of "index overlapping" in which, in the case
//! of multiple indices having the same name, the package from the "higher priority" index would be
//! picked. In this previous design, the package from the "cache index" would be picked, avoiding
//! re-downloading of packages.
//! 
//! However, this design of overlapping indices was abandoned because it made package resolution
//! unreliable and dependent on global state. Additionally, because an Index can only store and
//! manage metadata information, a separate Cache struct would've been needed anyway to manage
//! caching, making this design complex and complicated.
//! 
//! ## Current design
//! In this new design, there is a much clearer separation between storing package metadata
//! (handled strictly by Indices and the Cache's own Index, more on that later) and caching the
//! packages themselves (which is handled by the Cache struct). The Cache struct is responsible for
//! determining if a package has already been retrieved from the Internet, and coordinates using
//! cached package downloads.
//! 
//! At minimum, the Cache struct must be responsible for a directory which contains previously
//! downloaded packages from indices, and should deal with checksums and things like that to see if
//! a redownload is needed. Whenever a package is about to be downloaded, the Cache is there to see
//! if it really actually needs to be downloaded.
//! 
//! Additionally, a Cache should contain metadata on of all the packages which have no index on their
//! own; things like git repos or local file trees depended on by root packages.
//! Presumably this info would be contained in the Cache's own Index, made separate from the other
//! "remote" Indices. This index should NOT contain any info about packages obtained from other
//! indices. In this way, we can keep the metadata-handling to the Index, but have the Cache deal
//! with what it's best at: caches of actual packages.
//! 
//! By storing the Index of otherwise indexless packages under the Cache, we can special-case it
//! so that it isn't treated like other indices (i.e. for lockfiles, the Resolution is the og
//! DirectRes and not the IndexRes of the Cache's Index)
//! 
//! Git repos should prolly be cloned into the path of the Cache, but directories shouldn't be
//! copied; the index should just point that the location of the file.
//! 
//! One question you might have is "Why couple this Index to the Cache at all?" The reason is that
//! the Cache is responsible for anything retrieved from a DirectRes. For packages from Indices,
//! the Cache manages downloads of those packages so that it's not duplicated. For packages whose
//! Resolution is just a DirectRes, the Cache should be responsible for storing both those packages
//! and metadata about them, so it should have its own Index for those packages.
//! 
//! ### Future potential
//! This new design for the cache makes possible several desirable features which could easily be
//! implemented in the future.
//! 
//! ### "Airplane mode"
//! If a user does not want to access the Internet to resolve packages, `elba` can limit itself
//! to only using the packages provided by the Cache.
//! 
//! ### Vendoring
//! In order to vendor packages, `elba` can create a new Cache in the project directory and require
//! that all packages originate from the vendor directory (basically airplane mode + custom cache
//! directory). Directory dependencies should be copied into the Cache directory unconditionally.
//! 
//! ### Build caching
//! If we want to cache builds, we can just have a separate subfolder for ibcs.
//! Example directory structure:
//! 
//! ```none
//! cache
//!   |
//!   +-- index (for otherwise-indexless, DirectRes packages)
//!   |
//!   +-- dls
//!   |
//!   +-- builds
//! ```

use index::Index;
use std::path::PathBuf;

struct Cache {
    location: PathBuf,
    index: Index,
}

impl Cache {
    pub fn new(location: PathBuf) -> Self {
        unimplemented!()
    }
}