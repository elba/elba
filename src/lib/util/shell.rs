use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};
use std::fmt::Display;

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    None,
    Quiet,
    Normal,
    Verbose,
}

impl Serialize for Verbosity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match *self {
            Verbosity::None => "none",
            Verbosity::Quiet => "quiet",
            Verbosity::Normal => "normal",
            Verbosity::Verbose => "verbose",
        })
    }
}

impl<'de> Deserialize<'de> for Verbosity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "none" => Ok(Verbosity::None),
            "quiet" => Ok(Verbosity::Quiet),
            "normal" => Ok(Verbosity::Normal),
            "verbose" => Ok(Verbosity::Verbose),
            _ => Err(Error::custom(
                r#"invalid verbosity: must be one of: none, quiet, normal, verbose"#.to_string(),
            )),
        }
    }
}

impl Default for Verbosity {
    fn default() -> Self {
        Verbosity::Normal
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shell {
    pub verbosity: Verbosity,
}

impl Default for Shell {
    fn default() -> Self {
        Shell {
            verbosity: Verbosity::Normal,
        }
    }
}

impl Shell {
    pub fn println(self, status: impl Display, message: impl Display, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            println!("{:>12} {}", status, message);
        }
    }

    pub fn println_unindented(
        self,
        status: impl Display,
        message: impl Display,
        min_verbosity: Verbosity,
    ) {
        if self.verbosity >= min_verbosity {
            println!("{} {}", status, message);
        }
    }

    pub fn println_plain(self, message: impl Display, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            println!("{}", message);
        }
    }

    pub fn println_empty(self, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            println!();
        }
    }
}
