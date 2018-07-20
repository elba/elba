//! Deals with pretty-printing information to the user (i.e. not logs; those are for debugging
//! purposes, not for the user to see.)

// TODO: impl Shell should have warn, error, info

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

#[derive(Debug, Clone)]
pub struct Shell {
    verbosity: Verbosity,
}
