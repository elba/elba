# Indices

Indices tell matic where to get packages and what packages are available. matic
can load multiple indices at once, as specified in a configuration file. When
packages are specified in the manifest, matic will choose the package with that
name from the index with the highest priority. This is to prevent accidental
package overlaps between disparate packages leading to breaking everything.

However, during dependency resolution, matic will treat packages with the same
name from different indices as different packages. Because we have a
namespacing system, this might not be necessary; if a index has a package with
the same namespace and name as a package in another index, this is presumably
on purpose, because they want to "patch" this package. Implementing this change
would simply mean changing references to `PackageId` to `Name` in all files in
the `resolve` module and modifying `Index::select` to compare only name and version rather than the whole summary (which includes `Resolution`).
