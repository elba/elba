//! A package manager for the Idris language.
//!
#![feature(label_break_value)]

pub mod build;
pub mod cli;
pub mod package;
pub mod remote;
pub mod resolve;
pub mod retrieve;
pub mod util;
