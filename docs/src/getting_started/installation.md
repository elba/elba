## Installation

The easiest and most convenient way of installing elba is to use the pre-built binaries for elba, which can be downloaded from [GitHub Releases](https://github.com/elba/elba/releases). To install this way, just download the corresponding archive for your platform, extract the executable somewhere in your PATH, add `~/.elba/bin` to your PATH in order to execute elba-installed packages, and you're done!

> #### Note: `-gnu` vs `-musl`
>
> For Linux platforms, there are two varieties of binaries available: one
> suffixed with `-gnu` and the other suffixed with `-musl`. The `-gnu` binary is
> dynamically linked to the system libc, while the `-musl` binary is statically
> linked using musl.
>
> For most users, the `-gnu` binary should work fine, but if it doesn't, try
> using the `-musl` binary.

### Installing with Cargo

Because elba is written in Rust, it is available as an installable crate from [crates.io](https://crates.io). In order to install elba this way, you should have a copy of the Rust toolchain installed on your computer first. The process for this is explained on [the Rust website](https://www.rust-lang.org/en-US/install.html). Note that during installation, when prompted for which version of Rust should be installed, you should choose the **nightly version** of Rust. elba depends on certain features which can only be enabled in the nightly build of Rust.

Once you have Rust installed, installing elba is pretty self-explanatory:

```sh
$ cargo +nightly install elba
$ elba # should work!
```

Remember to add `~/.elba/bin` to your PATH to be able to run elba-installed packages.

### Building elba

Building elba from source is much the same process as installing it using cargo; the only difference is that instead of using a stable, versioned-crate available from crates.io, elba's source code is used directly. You'll still need to have the nightly version of the Rust toolchain installed (see the above section for more details). After that's done, download elba's source code and install it:

```sh
$ git clone https://github.com/elba/elba
$ cargo +nightly install --release
$ elba # should work!
```

Remember to add `~/.elba/bin` to your PATH to be able to run elba-installed packages.
