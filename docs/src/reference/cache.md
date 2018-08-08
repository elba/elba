## The Global Cache

elba uses a global cache to store downloaded packages, build packages in a temporary clean directory, and store built packages for future re-use. The structure of the global cache looks like the following:

```sh
~/.elba/cache or wherever
|
|-- bin
|   +-- .bins
|-- build
|   |-- a78bu877c78deadbeef...
|   +-- # snip
|-- indices
|   |-- d3237be53e69715112f...
|   +-- # snip
|-- src
|   |-- d2e4a311d3323b784ef...
|   +-- # snip
+-- tmp
    |-- a78bu877c78deadbeef...
    +-- # snip
```

### `bin`

This folder stores all of the installed binaries of elba packages, and should be added to the path. Additionally, an extra file `.bins` lives inside this directory, which maps from installed binaries to their corresponding package. This directory **should not** be touched by the user in any way.

### `build`

This folder stores the binary (i.e. `.ibc` file) outputs of library builds. elba globally caches the builds of all dependencies to avoid having to rebuild the same library over and over across different projects. Each built version of a package gets its own hash which encapsulates the entire environment under which the package was built (package dependencies, etc.), ensuring reproducible builds. This emulates the Nix package manager in some respects.

This folder and its subfolders are safe to delete, although it may cause rebuilds of some packages.

### `indices`

This folder stores the downloaded package indices as specified in elba's [configuration](../usage/configuration.md), with a hash corresponding to each different package index.

This folder and its subfolders are safe to delete; elba will redownload any needed indices on its next invocation.

### `src`

This folder stores the downloaded sources of packages. elba globally caches these to avoid having to redownload the same files over and over again.

For git repositories, using a different git ref will make elba treat it like a completely different package.

This folder and its subfolders are safe to delete, although it may cause having to redownload and rebuild some packages.

### `tmp`

This folder is a temporary build directory for packages, and is more of an implementation detail than anything else. Folders correspond to build hashes for packages, and the internal structure of these folders mirrors the `target/` directory of a local package build.

This folder and its subfolders can be safely deleted.

### Cleaning the cache

...can be accomplished with the following invocation:

```sh
$ elba clean
```

Doing so clears the `artifacts`, `build`, `indices`, `src`, and `tmp` directories.