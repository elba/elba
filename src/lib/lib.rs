#[macro_use]
extern crate failure;
#[macro_use]
extern crate nom;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate semver;
extern crate toml;

mod cli;
mod compiler;
mod config;
mod err;
mod package;
mod registry;
