# Changelog

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

