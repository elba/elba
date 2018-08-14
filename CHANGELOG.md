# Changelog

## Unreleased

### Changed

- elba's CLI output has been prettied up, and now respects
  the `--verbose` and `--quiet` flags.

- Fixed a bug where Idris would complain about "No ibc for _"
  when building a bin target (#14).

- Fixed a bug where any stdout output during code generation
  would cause the build process to error.

## 0.1.5

This release of elba fixes a bug with the REPL not loading
import paths correctly.

### Changed

- When launching the REPL, elba now adds the paths of all
  specified targets.

## 0.1.4

This release of elba fixes a bug with package initialization.

### Changed

- When creating a new library project, elba now adds the correct
  module by default.

## 0.1.3

This release of elba changes how it deals with tests.

### Changed

- elba can now build test targets without a library target needed.
  Tests always have access to all dependencies, dev-dependencies,
  and the files which share the same parent folder as the test's
  Main module. If no library target is found, elba will issue a
  warning.

## 0.1.2

This release of elba fixes a critical error with tarball resolutions
and cleans up error handling a bit.

### Changed

- elba now errors when downloading a tarball resolution if the hashes
  *do not* match, as opposed to before when it errored if they matched.

## 0.1.1

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

