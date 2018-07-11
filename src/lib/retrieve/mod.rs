//! Interfaces for retrieving packages (and information about them) from different sources.
//!
//! Packages can originate from several sources, which complicates getting metadata about them.
//! This module is responsible for smoothing over that process, as well as coordinating the actual
//! retrieval of packages from various different sources (hopefully in parallel).
