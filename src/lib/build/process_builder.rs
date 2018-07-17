use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::str;

use failure::{Error, ResultExt};
use shell_escape::escape;

use util::errors::ProcessError;

/// A builder object for an external process, similar to `std::process::Command`.
// TODO: move ProcessBuilder into utils once we need to invoke process
// by other modules
#[derive(Clone, Debug)]
pub struct ProcessBuilder {
    /// The program to execute.
    program: OsString,
    /// A list of arguments to pass to the program.
    args: Vec<OsString>,
    /// Any environment variables that should be set for the program.
    env: HashMap<String, Option<OsString>>,
    /// Which directory to run the program from.
    cwd: Option<OsString>,
}

impl fmt::Display for ProcessBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}", self.program.to_string_lossy())?;

        for arg in &self.args {
            write!(f, " {}", escape(arg.to_string_lossy()))?;
        }

        write!(f, "`")
    }
}

impl ProcessBuilder {
    pub fn new<T: AsRef<OsStr>>(cmd: T) -> Self {
        ProcessBuilder {
            program: cmd.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
        }
    }

    /// (chainable) Add an arg to the args list.
    pub fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut ProcessBuilder {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// (chainable) Add many args to the args list.
    pub fn args<T: AsRef<OsStr>>(&mut self, arguments: &[T]) -> &mut ProcessBuilder {
        self.args
            .extend(arguments.iter().map(|t| t.as_ref().to_os_string()));
        self
    }

    /// (chainable) Replace args with new args list
    pub fn args_replace<T: AsRef<OsStr>>(&mut self, arguments: &[T]) -> &mut ProcessBuilder {
        self.args = arguments
            .iter()
            .map(|t| t.as_ref().to_os_string())
            .collect();
        self
    }

    /// (chainable) Set the current working directory of the process
    pub fn cwd<T: AsRef<OsStr>>(&mut self, path: T) -> &mut ProcessBuilder {
        self.cwd = Some(path.as_ref().to_os_string());
        self
    }

    /// (chainable) Set an environment variable for the process.
    pub fn env<T: AsRef<OsStr>>(&mut self, key: &str, val: T) -> &mut ProcessBuilder {
        self.env
            .insert(key.to_string(), Some(val.as_ref().to_os_string()));
        self
    }

    /// (chainable) Unset an environment variable for the process.
    pub fn env_remove(&mut self, key: &str) -> &mut ProcessBuilder {
        self.env.insert(key.to_string(), None);
        self
    }

    /// Run the process, waiting for completion, and mapping non-success exit codes to an error.
    pub fn exec(&self) -> Result<(), Error> {
        let mut command = self.build_command();
        let exit = command.status().with_context(|_| {
            process_error(&format!("could not execute process {}", self), None, None)
        })?;

        if exit.success() {
            Ok(())
        } else {
            Err(process_error(
                &format!("process didn't exit successfully: {}", self),
                Some(&exit),
                None,
            ).into())
        }
    }

    /// On unix, executes the process using the unix syscall `execvp`, which will block this
    /// process, and will only return if there is an error. On windows this is a synonym for
    /// `exec`.
    #[cfg(unix)]
    pub fn exec_replace(&self) -> Result<(), Error> {
        use std::os::unix::process::CommandExt;

        let mut command = self.build_command();
        let error = command.exec();
        Err(CargoError::from(error)
            .context(process_error(
                &format!("could not execute process {}", self),
                None,
                None,
            ))
            .into())
    }

    /// On unix, executes the process using the unix syscall `execvp`, which will block this
    /// process, and will only return if there is an error. On windows this is a synonym for
    /// `exec`.
    #[cfg(windows)]
    pub fn exec_replace(&self) -> Result<(), Error> {
        self.exec()
    }

    /// Execute the process, returning the stdio output, or an error if non-zero exit status.
    pub fn exec_with_output(&self) -> Result<Output, Error> {
        let mut command = self.build_command();

        let output = command.output().with_context(|_| {
            process_error(&format!("could not execute process {}", self), None, None)
        })?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(process_error(
                &format!("process didn't exit successfully: {}", self),
                Some(&output.status),
                Some(&output),
            ).into())
        }
    }

    /// Converts ProcessBuilder into a `std::process::Command`
    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(cwd) = self.cwd.as_ref().map(Path::new) {
            command.current_dir(cwd);
        }
        for arg in &self.args {
            command.arg(arg);
        }
        for (k, v) in &self.env {
            match *v {
                Some(ref v) => {
                    command.env(k, v);
                }
                None => {
                    command.env_remove(k);
                }
            }
        }
        command
    }
}

/// Format process exit code with user friendly description
fn process_error(msg: &str, status: Option<&ExitStatus>, output: Option<&Output>) -> ProcessError {
    let exit = match status {
        Some(s) => status_to_string(s),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    return ProcessError {
        desc,
        exit: status.cloned(),
        output: output.cloned(),
    };

    #[cfg(unix)]
    fn status_to_string(status: &ExitStatus) -> String {
        use libc;
        use std::os::unix::process::*;

        if let Some(signal) = status.signal() {
            let name = match signal as libc::c_int {
                libc::SIGABRT => ", SIGABRT: process abort signal",
                libc::SIGALRM => ", SIGALRM: alarm clock",
                libc::SIGFPE => ", SIGFPE: erroneous arithmetic operation",
                libc::SIGHUP => ", SIGHUP: hangup",
                libc::SIGILL => ", SIGILL: illegal instruction",
                libc::SIGINT => ", SIGINT: terminal interrupt signal",
                libc::SIGKILL => ", SIGKILL: kill",
                libc::SIGPIPE => ", SIGPIPE: write on a pipe with no one to read",
                libc::SIGQUIT => ", SIGQUIT: terminal quite signal",
                libc::SIGSEGV => ", SIGSEGV: invalid memory reference",
                libc::SIGTERM => ", SIGTERM: termination signal",
                libc::SIGBUS => ", SIGBUS: access to undefined memory",
                #[cfg(not(target_os = "haiku"))]
                libc::SIGSYS => ", SIGSYS: bad system call",
                libc::SIGTRAP => ", SIGTRAP: trace/breakpoint trap",
                _ => "",
            };
            format!("signal: {}{}", signal, name)
        } else {
            status.to_string()
        }
    }

    #[cfg(windows)]
    fn status_to_string(status: &ExitStatus) -> String {
        status.to_string()
    }
}
