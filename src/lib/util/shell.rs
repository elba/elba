use std::fmt::Display;

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    None,
    Quiet,
    Normal,
    Verbose,
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
