# elba
[![Build Status](https://travis-ci.com/dcao/elba.svg?branch=master)](https://travis-ci.com/dcao/elba)

A modern and (hopefully!) fast package manager for Idris.

Development is currently in the pre-release stage; it's currently impossible to actually build a package with elba, but hopefully that'll change soon.

## Installation
`elba` is written in Rust, so the Cargo and the Rust compiler are required for building.

After those are installed, clone this repo and whack `cargo install`.

## Testing
One note for testing is that the integration tests will lock folders in the `data/` directory, since in a real-life application you don't want multiple instances of `elba` clobbering each others' work directory, so make sure to pass `--test-threads 1` to the test binary.