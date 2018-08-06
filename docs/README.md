# The elba Guide

"The elba Guide" is intended to be exactly what it says on the tin: a user-facing guide for using `elba` for day-to-day development tasks and understanding its functionality.

## Building

The guide uses [mdBook](https://github.com/rust-lang-nursery/mdBook) at the moment, so you'll have to install that first:

```
$ # install the rust toolchain somehow
$ # if you're building elba or installing with cargo it should be installed already
$ cargo install mdbook
```

After that, building the book is as simple as:

```
$ mdbook build
```

The book will be in the `book` directory, with the index at `book/index.html`.