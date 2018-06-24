//! Module `resolve` provides logic for resolving dependency graphs.

// The problem with building a tree of dependencies is that different versions of packages can have
// different dependencies, so you have to decide what version you choose right away, then you keep
// going.
//
// This is going to be awful.
// Helpful links:
// https://medium.com/@sdboyer/so-you-want-to-write-a-package-manager-4ae9c17d9527
// https://www.reddit.com/r/programming/comments/45ftk1/so_you_want_to_write_a_package_manager/czxxm43/
// Julia's resolver: uses the maxsum algorithm, a variant of sum-product (https://github.com/JuliaLang/Pkg.jl/tree/master/src/resolve)
// Dart's resolver (https://github.com/dart-lang/pub/blob/master/doc/solver.md)
// Rust's resolver (https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver)

pub mod context;

use package::PackageId;
use std::collections::HashSet;

use index::Index;

pub fn resolve(packages: &[PackageId], index: &mut Index, locked: &HashSet<PackageId>) {
    // let mut ctx = Context::new();

    unimplemented!()
}
