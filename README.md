# elba

[![Not-Windows Build
Status](https://travis-ci.com/elba/elba.svg?branch=master)](https://travis-ci.com/elba/elba)
[![Windows Build
Status](https://ci.appveyor.com/api/projects/status/j2pk9krx63o1dpdv?svg=true)](https://ci.appveyor.com/project/dcao/elba)

[Elba](https://www.elba.pub/) is a modern and (hopefully!) fast package manager for
[Idris](https://www.idris-lang.org). Supplemental information and alternatives
can be found at [this blog post](http://cao.st/posts/elba/).

## Installation

There are two options for installing elba:

1.  Download the pre-built binary corresponding to your platform from
    GitHub Releases and place it in your PATH.
3.  Manually build and install elba yourself using the source code with
    `git clone https://github.com/elba/elba.git && cd elba && cargo install --path .`.
    
To build, elba requires the latest nightly Rust.

## Documentation

[The elba Guide](https://elba.readthedocs.io) is intended to be the
ultimate source of information on using elba and understanding its
functionality.

Documentation for elba-the-Rust-library is hosted at
[docs.rs/elba](https://docs.rs/elba).

## Contributing

Contributions are welcome; you can create an issue if you have a
feature request or a bug report, and you can submit a pull request if you'd like
to address a sore spot yourself. If you'd like to implement a large feature,
please either leave a comment on an existing issue or create a new issue for
that feature.

Discussion happens on the [Matrix channels](https://matrix.to/#/+elba:matrix.org) 
of the elba community; discussion about this command-line utility specifically happen
in the [relevant channel](https://matrix.to/#/#elba-cli:matrix.org).

## License

elba itself is distributed under the [MIT License](./LICENSE).
