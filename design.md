# A possible design for this tool:

Everything is installed with the install command.

```
$ olwen install idris // installs the idris compiler
$ olwen install idris-lsp // future lsp server for idris
$ olwen install lightyear
```

Packages are of two types: normal or tooling.

All packages must depend on a proper version of the Idris compiler, except for
the Idris compiler itself, which is special-cased.

A normal package is simply copied to the pkgdir of the chosen Idris compiler's
install.

- [ ] TODO figure out tooling package's privileges


## The manifest
All package dependencies, whether normal or tooling, are listed under
[dependencies].
Example:

```toml
[package]
name = "test"
authors = ["A <a@a.a>", "B <b@b.b>"]

[dependencies]
idris = "1.3" # resolved like with cargo
idris-jvm = "1.1"
other = { git = "https://github.com/cmdd/cool", tag = "v1.1.1" }

[lib]
mods = [
    "A",
    "A.B",
    "A.B.C",
    "A.B.D",
]

[[bin]]
name = "bin1"
mod = "A.Main"

[[bin]]
name = "bin2"
mod = "A.B.Main"
```
