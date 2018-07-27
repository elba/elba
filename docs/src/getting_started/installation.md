# Installation

At the moment, because elba is in active development, the only way to install elba is by building it yourself and installing it that way. In the future, binaries will be available through [GitHub Releases](https://github.com/dcao/elba/releases) which will mostly obviate the need for manually building elba.

## Building elba

elba is written in Rust, so in order to build elba, you have to get the Rust toolchain installed first. The process for this is explained on [the Rust website](https://www.rust-lang.org/en-US/install.html). Note that during installation, when prompted for which version of Rust should be installed, you should choose the **nightly version** of Rust. elba depends on certain features which can only be enabled in the nightly build of Rust.

After Rust has been installed, clone the elba repo and install it with the following incantation:

```
$ cargo install
```