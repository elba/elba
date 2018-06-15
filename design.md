# A possible design for this tool:

## The manifest
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

## Unsolved questions
- [ ] for "mod", should it be by idris module (dot) or file (slash)?
