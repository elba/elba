//! A package manager for the Idris language.

#![feature(async_await)]

pub mod build;
pub mod cli;
pub mod package;
pub mod remote;
pub mod resolve;
pub mod retrieve;
pub mod util;
