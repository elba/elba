//! Library `matic` defines a package manager for the Idris language.
//!
//! Human-editable configuration files are in TOML. Things that people shouldn't touch are in JSON.

#[macro_use]
extern crate failure;
extern crate im;
#[macro_use]
extern crate nom;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate semver;
extern crate serde_json;
extern crate toml;
extern crate url;
extern crate url_serde;

pub mod cli;
pub mod compiler;
pub mod config;
pub mod err;
pub mod index;
pub mod package;
pub mod resolve;
