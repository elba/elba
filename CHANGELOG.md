# Changelog

## [0.3.3]

- Support iPKG manifest (#25)

- Fix indirect local dependency resolution. (#65)

- Add .dirlock to .gitignore

- Fix warning of unable to clean build directory on Windows.

- Allow assigning `--threads` argument.

- Kill compiler on elba exit.

## [0.3.2]

This release decreases the number of compilation tasks in parallel
so as to reduce the chance that Idris compiler breaks down on high load.

## [0.3.1]

This release reduces the build time and comes with new command line
output appearance.

### Changed

- Bin targets are able to import lib target of the same project, if there
is one.

- Lib target will **not** codegen artifacts by default when publish.

- Fix bugs where bin target codegen fails. (#61)

## [0.3.0]

This is a major release which adds many new features and polishes many
old ones. In particular, this release adds proper support for package
repositories (deemed "indices" and "registries") by adding many new
commands and manifest fields for interacting with registries, and
generally making the index/registry system more robust.

This release also provides experimental support for the Blodwen
compiler; in the future, more robust support will be added.

### Added

- **BREAKING CHANGE**: A new format for the `indices` key: index aliases
  can now be specified in elba's configuration, which propagate to
  packages and commands which take an `--index` flag.

- New commands for interacting with package registries: `package` for
  creating tested tarballs of packages, `publish` for publishing a
  package to an index, `yank` for disallowing new packages to
  depend on a published package, `search` for searching through
  packages, and `login` for saving credentials for an index (#31, #35).
  
- A new command `check`, along with associated alias `c`, which checks all
  Idris source files but doesn't build any artifacts for them.
  
- A new command `add` for adding a dependency to the current project (#51).
  
- New fields to the manifest for use with package registries:
  `package.description`, `package.homepage`, `package.repository`,
  `package.readme`, and `package.keywords` (#30, #42).
  
- A new manifest field `package.exclude`, which specifies which files
  should be ignored when checking for file changes or building a
  package. Also includes entries in `.gitignore`.
  
- A new manifest field `scripts` along with a command `script`,
  which allows for running arbitrary shell commands/scripts. A special
  script, `prebuild`, is always run (if present) before a package is
  built; the build will fail if the command doesn't return a success.

- A new config key `directories.data` which specifies the folder where
  elba will store data files.
  
- A new config key `compiler` to specify the name of the compiler
  executable.
  
- Preliminary support for the Blodwen compiler. (Note that because the
  compiler is still so barebones, it can't build any packages with
  dependencies or executables)
  
- A new direct source for packages: a package registry (#37).

- A flag `--unyank` to the `elba yank` command, allowing for unyanking
  packages.

### Changed

- **BREAKING CHANGE**: the syntax for specifying a package from a custom
  index has now changed to be consistent with the docs:
  
  ```toml
  "index/explicit" = { version = ">= 0.1.0", index = "index+dir+../index" }
  ```

- **BREAKING CHANGE**: the `backend` field in `index.toml` is now
  `registry`.

- "Remote backends" or "index backends" are now "registries."

- If a registry is specified for an index, the locations of each
  package defaults to the registry (#37).

- Made elba check for literate Idris files when looking for modules
  (#28).

- Allowed passing options to the Idris compiler with `--`.

- Fixed bug where `elba repl` wouldn't remove ibc files.

- Fixed bug where elba would allow an empty group and/or name.

- Fixed bug where, in offline mode, elba would not find packages which
  were a subfolder of another package.

- Added extra Idris and backend options as part of the build hash.

- Allow for virtual packages with no build targets.

## [0.2.0]

This is a major release of elba which polishes up the 0.1.x release
series, fixing up a multitude of different bugs, adding commands
where appropriate, and making major enhancements to ergonomics
(most notably major improvements to how git dependencies work,
the addition of an offline mode, functionality enhancements to
binary targets, and added defaults for manifests).

### Added

- The `update` command, which updates all of the packages in the
  lockfile (or certain packages, based on a command line flag
  `--package`)
  
- The `print-config` command, which does exactly what it says
  on the tin

- More complex logic for dealing with binary target paths; the end
  result is that files which don't specify a `Main.main` function
  can now still be used as binary targets, since elba can generate
  files with a `Main.main` function on-the-fly.

- A flag `--offline` to enable offline mode. In offline mode, elba
  will skip downloading anything entirely (to avoid waiting for
  timeouts) and will only use packages available in the elba's
  local cache when resolving dependencies (#23).

- A flag `--vcs` for `elba new` and `elba init`, to determine if
  elba should automatically create a vcs repo when creating a
  project.
  
- A flag `--debug-log` to print not-pretty debug logs for build
  commands.

- Sensible defaults for target paths and tests (#17).

- Separate configuration options for storing binary files and elba-
  internal cache files, if you're really intent on keeping your
  home directory clean.

- A new flag `--ide-mode` to the `elba repl` subcommand, for
  running an ide server for the current root package.

### Changed

- elba will now look through all of the current directory's
  ancestors for a manifest file, rather than just the current
  directory.
  
- When (un)installing packages, elba will now error if a spec
  is ambiguous.

- When generating lockfiles, for packages located at git repos,
  elba will lock the git repo to a specific commit, rather than
  always trying to get the latest revision of the git repo (see
  the docs for more details).

- During dependency retrieval, a package index will only be
  updated iff the manifest depends on a package which cannot
  be found in the cached indices or the version specified in
  the manifest conflicts with that in the lockfile.

- Reduced HTTP timeout from 30 to 10 seconds.

- Prettied up and fixed elba's CLI output; elba now also respects
  the `--verbose` and `--quiet` flags (#12).

- Fixed a bug where Idris would complain about `No ibc for...`
  when building a bin target (#14).

- Fixed a bug where any stdout output during code generation
  would cause the build process to error.

- Fixed bugs with config file parsing; elba can actually read
  the `term.verbosity` key, and elba is more lenient when it
  comes to missing keys in configuration.

- Fixed a bug with "unknown reference" errors when generating
  test binaries.

- Fixed a bug with not erroring during a codegen invocation
  when we should.

- elba no longer pollutes the home directory as much by default; 
  elba's internal cache files are stored in a platform-specific
  cache folder if another folder isn't specified in the config.
  elba still uses `~/.elba/bin` for globally-installed binaries,
  however.

- elba now takes the version of the current compiler into account
  when deciding if it needs to rebuild a package.
  
### Removed

- **BREAKING CHANGE**: The `lock` command, which has been superseded
  by `update`.

## [0.1.5]

This release of elba fixes a bug with the REPL not loading
import paths correctly.

### Changed

- When launching the REPL, elba now adds the paths of all
  specified targets.

## [0.1.4]

This release of elba fixes a bug with package initialization.

### Changed

- When creating a new library project, elba now adds the correct
  module by default.

## [0.1.3]

This release of elba changes how it deals with tests.

### Changed

- elba can now build test targets without a library target needed.
  Tests always have access to all dependencies, dev-dependencies,
  and the files which share the same parent folder as the test's
  Main module. If no library target is found, elba will issue a
  warning.

## [0.1.2]

This release of elba fixes a critical error with tarball resolutions
and cleans up error handling a bit.

### Changed

- elba now errors when downloading a tarball resolution if the hashes
  *do not* match, as opposed to before when it errored if they matched.

## [0.1.1]

This release of elba modifies the behavior of elba to interact better
with package indices overall.

### Changed

- elba now includes a default package index, located at the GitHub repo
  `elba/index`.

- elba now redownloads all package indices every time it is invoked,
  regardless of if they have been cached or not.

## 0.1.0

This is the initial release of elba, and contains most of the basic
functionality needed for Idris development: building, testing, and
installing packages; developing them interactively; and depending on
other packages.

### Added

- Commands for creating packages, building packages (generating a lockfile
  and building all targets), testing packages, and (un)installing packages.

[Unreleased]: https://github.com/elba/elba/compare/0.3.2...HEAD
[0.3.2]: https://github.com/elba/elba/compare/0.3.1...0.3.2
[0.3.1]: https://github.com/elba/elba/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/elba/elba/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/elba/elba/compare/0.1.5...0.2.0
[0.1.5]: https://github.com/elba/elba/compare/0.1.4...0.1.5
[0.1.4]: https://github.com/elba/elba/compare/0.1.3...0.1.4
[0.1.3]: https://github.com/elba/elba/compare/0.1.2...0.1.3
[0.1.2]: https://github.com/elba/elba/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/elba/elba/compare/0.1.0...0.1.1
