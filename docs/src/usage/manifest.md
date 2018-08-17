## The Manifest

In order to keep track of package metadata like the name of a package and what targets should be built for that particular package, elba uses an `elba.toml` manifest file. This file is divided into several different sections which each provide information to elba about the package in question.

### `[package]`

The first and most important section of the manifest is the `[package]` section, which lists all of the metadata about the package. A complete example of a `[package]` section is shown below:

```toml
[package]
name = "jsmith/elba"
version = "0.1.0"
authors = ["John Smith <dcao@example.com>"]
description = "The best package ever released"
license = "MIT"
```

The namespaced name and version are the two most important parts of this specification. The name must contain a group (i.e. a namespace) and a name, separated by a slash, or else the manifest will fail to parse. Additionally, the name can only contain alphanumeric characters, hyphens, and underscores. Internally, elba ignores case and treats hyphens and underscores equally when deciding if two names are identical. The version must follow [Semantic Version guidelines](https://semver.org/). Additionally, the package section contains fields to indicate the authors of the package and the license which the code falls under. The authors section can be left empty, and each author should follow the format `name <email>` (this is just a helpful convention to follow). The license field can be omitted entirely, as can the description.

> #### Why namespacing?
>
> Having to supply a namespace to all package names might seem like unnecessary work, but it has its benefits; this design decision to require all package names to be namespaced was borne out of observations of other package ecosystems where the lack of namespaces lead to significant problems down the line. In particular, namespaced packages provide the following benefits:
>
> - Packages which should belong to a single "group" or are a part of a single ecosystem can easily be grouped together, rather than using ad-hoc kinda-sorta-namespacing by prefixing all related packages with some name, which any untrusted package uploader can do
>
> - Name-squatting becomes less of an issue; instead of one global `http` package in a package index, there are now separate `jsmith/http` or `whatever/http` packages
>
> - Namespacing doesn't stop people from coming up with "creative" names; you can still create a package called `jsmith/unicorns_and_butterflies` if you'd like.

### `[dependencies]` and `[dev_dependencies]`

These sections of the manifest are mostly self-explanatory; they're a place where you can specify the dependencies that your package needs. All packages in the `[dependencies]` section will be loaded for every target of the package, while the packages in the `[dev_dependencies]` section will only be loaded for test targets.

elba dependencies can originate from one of three places: a package index (think RubyGems or crates.io), in which the package is identified by its version and package index (defaulting to the first package index specified in the [config file](./configuration.md); a git repository, in which the package is identified by the url of the git repo and a git ref name (defaulting to "master"); and a directory tree, in which the package is identified by its path.

An example of these sections and all the types of dependencies is shown below:

```toml
# deps used for all targets
[dependencies]
"index/version" = "0.1.5" # uses the default index (i.e. the first specified one in configuration)
"index/explicit" = { version = "0.1.5", index = "index+dir+../index" } # uses the index specified
"directory/only" = { path = "../awesome" } # uses the package in the path specified

# deps only used for the test targets
[dev_dependencies]
"git/master" = { git = "https://github.com/doesnt/exist" } # uses the master branch
"git/explicit" = { git = "https://github.com/doesnt/exist", branch = "beta" } # "branch" can be an arbitrary git ref: a tag, commit, etc.
```

elba's syntax for versioning has [several idiosyncrasies of its own](../reference/dependencies.md), but the tl;dr version is that elba will always pick a version of that package which is greater than or equal to and semver compatible with the version specified.

For more information about package indices, see the [relevant reference page](../reference/indices.md).

### `[targets]`

In order to know which files to build and how to build them, elba manifest files also must specify a `[targets]` section. There are three types of targets which elba can build:

- A **library target** is exactly what it sounds like: a built library of ibc files which can be used and imported by other elba packages. Each package can only export a single library target; attempting to specify multiple library targets will result in a manifest parsing error. The syntax for a library target is as follows:

  ```toml
  [targets.lib]
  # The path to the library - defaults to "src"
  path = "src"
  # The list of files which should be exported and made available for public use
  mods = [
    "Awesome.A", # the file src/Awesome/A.idr
    "Control.Zygohistomorphic.Prepromorphisms", # the file src/Control/Zygohistomorphic/Prepromorphisms.idr
  ]
  # Optional flags to pass to the compiler
  idris_opts = ["--warnpartial"]
  ```

  The `path` key should be a **sub-path** of the package; it cannot reference parent or absolute directories of the package. During the build process, all of the files under the `path` sub-path will be used to build the library and export the Idris bytecode files corresponding to the items in `mods`.

- A **binary target** is a binary which should be generated based on a Main module. Packages can have as many binary targets as they please; by default, all binary targets are built/installed in an `elba build` or `elba install` invocation, but this can be changed with the `--bin` flag. The syntax for a binary target is as follows:

  ```toml
  [[targets.bin]]
  # The name of the output binary
  name = "whatever"
  # The path to the folder containing the binary source - defaults to "src"
  path = "src/bin"
  # The path to the Main module
  main = "Whatever" # corresponds to src/bin/Whatever.idr
  # Optional flags to pass to the compiler
  idris_opts = ["--warnpartial"]
  ```

  The `name`, and `idris_opts` fields should be self-explanatory, but the `path` and `main` arguments have some more nuance to them. In order to maintain backwards compatibility while providing maximum flexibility, elba follows several steps to resolve the location of a binary target. It's pretty hard to explain these steps, but examples are much easier to follow:

  ```toml
  # Example 1: strict subpath specified in main, with folders separated by
  # slashes. extension left unspecified.
  main = "bin/Whatever/Module"
  # corresponds to bin/Whatever/Module.idr if it exists, otherwise uses
  # src/bin/Whatever/Module.idr because of the default `path` value; this file
  # should have a function Main.main

  # Example 2: main uses dots instead of slashes to separate folders, and
  # includes an idr extension
  main = "Whatever.Module.idr"
  # because this is not a valid subpath (uses dots instead of slashes),
  # this corresponds to the first of the following files which exists:
  # - src/Whatever/Module/idr.idr (treat the last section as a module)
  # - src/Whatever/Module.idr (treat the last section as an extension:
  #                            applies to the "idr" extension only)
  # this file should have a function Main.main

  # Example 3: strict subpath specified with non-"idr" extension
  main = "bin/Whatever/Module.custom"
  # corresponds to the first of the following files which exists:
  # - bin/Whatever/Module.idr
  # - src/bin/Whatever/Module.idr (due to the default `path` value)
  # in both cases, this file should have a function `Module.custom : IO ()`,
  # which will be used as the main function

  # Example 4: non-subpath combined with custom path and non-"idr" extension
  path = "bin"
  main = "Whatever.Module.custom"
  # corresponds to the first of the following files which exists:
  # - bin/Whatever/Module/custom.idr (treat the last section as a module)
  # - bin/Whatever/Module.idr (treat the last section as a function in a parent module)
  # if this corresponds to `bin/Whatever/Module.idr`, then the file should have a
  # function `Whatever.Module.custom : IO ()`, which will be used as the main
  # function
  ```

- A **test target** shares many similarities with a binary target: the syntax is almost exactly the same, and a single package can have multiple test targets. Indeed, in elba, tests are just executables which return **exit code 0 on success** and **any other exit code on failure**. The distinguishing features of a test target are as follows:

  - The `path` value for test targets defaults to `tests/` instead of `src/`

  - The `name` value defaults to the value in main, with slashes and periods replaced with underscores and `test-` prepended. 

  - Test targets have access to (i.e. can import from) **all dev dependencies** along with **the package's own library target**.

    This means that if you want to test a library target, you don't have to do anything special, just import your library like you normally would.

    If you want to test a binary, you can still do this, since a test will be built with all of the files in the same directory as the test's Main module, so if you put your test's Main module in the folder as a binary target, you can import everything that your binary target can from within the test.

  - Test targets can be automatically built and run in one shot using the command `elba test`.
  
  You'll note that the syntax for specifying a test target is remarkably similar to that for specifying a binary target:

  ```toml
  # The name of the output test binary
  name = "test-a"
  # The path to the test's Main module
  main = "tests/TestA.idr"
  # Optional flags to pass to the compiler
  idris_opts = ["--warnpartial"]
  ```

An elba package **must** specify either a lib target or a bin target, or else the manifest will be rejected as invalid.

For local packages, after building, all binaries will be output to the `target/bin` folder, and any library will be output to the `target/lib` folder. Additionally, for libraries, if you pass the `--lib-cg` flag, elba will use the codegen backend specified (or the C backend by default) and any export lists specified in the exported files of the library to create output files under `target/artifacts/<codegen name>` (for more information on export lists and the like, see [this test case in the Idris compiler](https://github.com/idris-lang/Idris-dev/tree/master/test/ffi006)).

### `[workspace]`

The last section in the manifest is the workspace section, used to indicate subprojects in the current directory. At the moment, the only use for this field is to indicate to elba the location of a package in a subdirectory (for example, with if a git repo has a package located in some subdirectory). Adding a package to the local workspace *does not* automatically add it as a local dependency of the package, nor does it cause the workspace packages to be automatically built when the root package is built. To add local directories as dependencies, they must manually be specified in either the `[dependencies]` or `[dev_dependencies]` sections.

Note that the directory of every package must be a **sub-path**; it cannot refer to an absolute directory or a directory above the root package.

An example workspace section is shown below:

```toml
[workspace]
"name/one" = "pkgs/one"
"other/pkg" = "wherever/youd/like"
```

Note that a a `[workspace]` section can stand alone and be parsed as a valid manifest if there is no package in the root directory.

### An aside: the lockfile

In order to keep track of the dependency tree and create reproducible builds,
elba uses a lockfile called `elba.lock`. This lockfile **should not be
modified** in any way, as it can lead to unpredictable results during the build
process.

The lockfile will not change so long as all of the packages in the lockfile
satisfy the requirements of the manifest and of its transitive dependencies. For
git repositories, the lockfile will lock a package to a commit, which won't
change given that the following conditions hold:

  - If the manifest references a branch, the locked commit must be contained
    within that branch.

  - If the manifest references a specific tag or commit, the locked commit must
    be equal to that tag or commit.
