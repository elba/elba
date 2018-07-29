# elba: A Guide

"elba: A Guide" is intended to be a user-facing guide for using `elba` for day-to-day development tasks. It might include information for devs too (who knows).

## Building

The guide uses [mdBook](https://github.com/rust-lang-nursery/mdBook) at the moment, so you'll have to install first:

```
$ # install the rust toolchain somehow
$ # if you're building elba it should be installed already
$ cargo install mdbook
```

After that, building the book is as simple as:

```
$ mdbook build
```

The book will be in the `book` directory, with the index at `book/index.html`.