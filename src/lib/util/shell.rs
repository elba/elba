//! Deals with pretty-printing information to the user (i.e. not logs; those are for debugging
//! purposes, not for the user to see.)

use super::config::Verbosity;

// TODO: impl Shell should have warn, error, info

#[derive(Debug, Clone)]
pub struct Shell {
    try_color: bool,
    verbosity: Verbosity,
}

impl Shell {}
