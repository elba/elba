#![feature(nll)]

//! A package manager for the Idris language.

#[macro_use]
extern crate failure;
extern crate flate2;
#[macro_use]
extern crate indexmap;
extern crate itertools;
#[macro_use]
extern crate nom;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate semver;
extern crate serde_json;
extern crate sha2;
extern crate symlink;
extern crate tar;
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
pub mod retrieve;
