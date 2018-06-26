# Indices

Indices tell matic where to get packages and what packages are available. matic
can load multiple indices at once, as specified in a configuration file. When
packages are specified in the manifest, matic will choose the package with that
name from the index with the highest priority. This is to prevent accidental
package overlaps between disparate packages leading to breaking everything.

However, during dependency resolution, matic will treat packages with the same
name from different indices as different packages. Because we have a
namespacing system, this might not be necessary; if an index has a package with
the same namespace and name as a package in another index, this is presumably
on purpose, because they want to "patch" this package. Implementing this change
would simply mean changing references to `PackageId` to `Name` in all files in
the `resolve` module and modifying `index::Dep` to use only `Name` rather than
`PackageId` (which includes `Resolution`).

`index::Index::select` compares Summaries rather than just `Name`s and
`Version`s since it deals with selecting a specific version of a package from a
specific place. The logic for "overlapping indices" would be in a different
method on `Indices` and `Index`, and would also entail changing `Dep` to hold
`Name`s instead of `PackageId`s.
