//! Library `matic` defines a package manager for the Idris language.
//!
//! Human-editable configuration files are in TOML. Things that people shouldn't touch are in JSON.

#[macro_use]
extern crate failure;
#[macro_use]
extern crate nom;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate semver;
extern crate toml;
extern crate url;
extern crate url_serde;

mod cli;
mod compiler;
mod config;
mod err;
mod package;
mod registry;
