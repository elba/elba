//! Utilities for creating and interacting with commands, including prettying up output and
//! building command-line invocations.

use crate::util::{error::Result, fmt_output, read2};
use failure::{bail, ResultExt};
use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};
use std::{
    borrow::Cow,
    fmt::Display,
    process::{Command, ExitStatus, Output, Stdio},
};

/// The requested verbosity of output
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    None,
    Quiet,
    Normal,
    Verbose,
}

impl Serialize for Verbosity {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
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
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
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
            let message = format!("{}", message);
            println!("{} {}", status, message);
        }
    }

    pub fn println_plain(self, message: impl Display, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            let message = format!("{}", message);
            if !message.trim().is_empty() {
                println!("{}", message);
            }
        }
    }

    pub fn print_plain(self, message: impl Display, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            let message = format!("{}", message);
            if !message.trim().is_empty() {
                print!("{}", message);
            }
        }
    }

    pub fn println_empty(self, min_verbosity: Verbosity) {
        if self.verbosity >= min_verbosity {
            println!("");
        }
    }
}

/// A group of command-line outputs.
#[derive(Default)]
pub struct OutputGroup(pub Vec<Output>);

impl OutputGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, out: Output) {
        self.0.push(out);
    }

    pub fn stdout(&self) -> impl Iterator<Item = Cow<'_, str>> {
        self.0.iter().map(|x| String::from_utf8_lossy(&x.stdout))
    }

    pub fn stderr(&self) -> impl Iterator<Item = Cow<'_, str>> {
        self.0.iter().map(|x| String::from_utf8_lossy(&x.stderr))
    }

    pub fn statuses(&self) -> Vec<ExitStatus> {
        self.0.iter().map(|x| x.status).collect()
    }

    /// Returns the index of the first failed output, or None.
    pub fn status(&self) -> Option<usize> {
        for (i, o) in self.0.iter().enumerate() {
            if !o.status.success() {
                return Some(i);
            }
        }

        None
    }
}

impl From<Output> for OutputGroup {
    fn from(f: Output) -> OutputGroup {
        OutputGroup(vec![f])
    }
}

/// An extension trait for Commands
pub trait CommandExt {
    // This is taken from Cargo (MIT licensed):
    // https://github.com/rust-lang/cargo/blob/76ce4df/src/cargo/util/process_builder.rs#L196
    /// Execute a command, passing each line of stdout and stderr to the supplied callbacks, which
    /// can mutate the string data.
    ///
    /// If any invocations of these function return an error, it will be propagated.
    ///
    /// Optionally, output can be passed to errors using `capture_output`.
    fn exec_streaming(
        &mut self,
        on_stdout_line: &mut impl FnMut(&str) -> Result<()>,
        on_stderr_line: &mut impl FnMut(&str) -> Result<()>,
        capture_output: bool,
    ) -> Result<Output>;
}

impl CommandExt for Command {
    fn exec_streaming(
        &mut self,
        on_stdout_line: &mut impl FnMut(&str) -> Result<()>,
        on_stderr_line: &mut impl FnMut(&str) -> Result<()>,
        capture_output: bool,
    ) -> Result<Output> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        self.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut callback_error = None;
        let status = (|| {
            let mut child = self.spawn()?;
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            read2(out, err, &mut |is_out, data, eof| {
                let idx = if eof {
                    data.len()
                } else {
                    match data.iter().rposition(|b| *b == b'\n') {
                        Some(i) => i + 1,
                        None => return,
                    }
                };
                {
                    // scope for new_lines
                    let new_lines = if capture_output {
                        let dst = if is_out { &mut stdout } else { &mut stderr };
                        let start = dst.len();
                        let data = data.drain(..idx);
                        dst.extend(data);
                        &dst[start..]
                    } else {
                        &data[..idx]
                    };
                    for line in String::from_utf8_lossy(new_lines).lines() {
                        if callback_error.is_some() {
                            break;
                        }
                        let callback_result = if is_out {
                            on_stdout_line(line)
                        } else {
                            on_stderr_line(line)
                        };
                        if let Err(e) = callback_result {
                            callback_error = Some(e);
                        }
                    }
                }
                if !capture_output {
                    data.drain(..idx);
                }
            })?;
            child.wait()
        })()
        .context(format!("could not execute process {:?}", self))?;

        let output = Output {
            stdout,
            stderr,
            status,
        };

        {
            if let Some(e) = callback_error {
                return Err(e);
            } else if !output.status.success() {
                bail!(
                    "process didn't exit successfully: {:?} (code {:?})\n{})",
                    self,
                    Some(output.status),
                    if capture_output {
                        fmt_output(&output)
                    } else {
                        String::new()
                    },
                )
            }
        }

        Ok(output)
    }
}
