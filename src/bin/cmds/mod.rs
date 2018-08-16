mod build;
mod clean;
mod doc;
mod init;
mod install;
mod lock;
mod new;
mod repl;
mod test;
mod uninstall;

use clap::{App, ArgMatches};
use elba::util::{
    config::{Backend, Config},
    errors::Res,
};
use failure::{Error, ResultExt};
use itertools::Itertools;
use slog::{Discard, Logger};
use std::process::Command;

pub type Exec = fn(&mut Config, &ArgMatches) -> Res<String>;

pub fn subcommands() -> Vec<App<'static, 'static>> {
    vec![
        build::cli(),
        clean::cli(),
        doc::cli(),
        init::cli(),
        install::cli(),
        new::cli(),
        lock::cli(),
        repl::cli(),
        test::cli(),
        uninstall::cli(),
    ]
}

pub fn execute_internal(cmd: &str) -> Option<Exec> {
    match cmd {
        "build" => Some(build::exec),
        "clean" => Some(clean::exec),
        "doc" => Some(doc::exec),
        "init" => Some(init::exec),
        "install" => Some(install::exec),
        "new" => Some(new::exec),
        "lock" => Some(lock::exec),
        "repl" => Some(repl::exec),
        "test" => Some(test::exec),
        "uninstall" => Some(uninstall::exec),
        _ => None,
    }
}

pub fn execute_external(cmd: &str, args: &ArgMatches) -> Result<String, Error> {
    let ext_args: Vec<&str> = args
        .values_of("")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);

    Command::new(cmd)
        .args(&ext_args)
        .spawn()
        .with_context(|e| {
            format_err!(
                "failed to spawn external command `elba-{} {}`:\n{}",
                cmd,
                ext_args.iter().join(" "),
                e
            )
        })?.wait_with_output()
        .with_context(|e| {
            format_err!(
                "failed to get output of external command `elba-{} {}`:\n{}",
                cmd,
                ext_args.iter().join(" "),
                e
            )
        })?;

    Ok(format!(
        "finished executing `elba-{} {}`",
        cmd,
        ext_args.iter().join(" ")
    ))
}

pub fn logger(_c: &mut Config) -> Logger {
    Logger::root(Discard, o!())
}

pub fn match_backends(c: &mut Config, args: &ArgMatches) -> Backend {
    let mut backend = args
        .value_of_lossy("backend")
        .and_then(|x| {
            let x = x.into_owned();
            c.get_backend(&x)
        }).unwrap_or_else(|| c.default_backend());

    // We do this because we want to preserve the name of the backend, even if it wasn't in the
    // config
    if let Some(x) = args.value_of_lossy("backend") {
        backend.name = x.into_owned();
    }

    backend.portable = if args.is_present("portable") {
        true
    } else if args.is_present("non-portable") {
        false
    } else {
        backend.portable
    };

    if let Some(x) = args.values_of_lossy("be-opts") {
        backend.opts = x;
    }

    backend
}

pub fn match_threads(_c: &mut Config, args: &ArgMatches) -> u32 {
    args.value_of("build-threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

mod args {
    use clap;

    type Arg = clap::Arg<'static, 'static>;

    pub fn target_lib() -> Arg {
        Arg::with_name("lib")
            .long("lib")
            .help("Makes the command apply to the library target")
    }

    pub fn target_bin() -> Arg {
        Arg::with_name("bin")
            .long("bin")
            .takes_value(true)
            .min_values(0)
            .help("The names of the binaries to which the command should apply (or all if no argument is provided)")
    }

    pub fn target_test() -> Arg {
        Arg::with_name("test")
            .long("test")
            .takes_value(true)
            .min_values(0)
            .help("The names of the tests to which the command should apply (or all if no argument is provided)")
    }

    pub fn build_threads() -> Arg {
        Arg::with_name("build-threads")
            .long("build-threads")
            .short("j")
            .takes_value(true)
            .number_of_values(1)
            .help("The number of threads to use to build")
    }

    pub fn backends() -> Vec<Arg> {
        vec![
            Arg::with_name("backend")
                .long("backend")
                .conflicts_with("portable-backend")
                .takes_value(true)
                .number_of_values(1)
                .help("Specifies the codegen backend to use during code generation"),
            Arg::with_name("portable")
                .long("portable")
                .help("Treat the codegen backend as if it were portable"),
            Arg::with_name("non-portable")
                .long("non-portable")
                .conflicts_with("portable")
                .help("Treat the codegen backend as if it were non-portable"),
            Arg::with_name("be-opts")
                .long("be-opts")
                .takes_value(true)
                .min_values(1)
                .help("Options to pass to the codegen backend"),
        ]
    }

    pub fn offline() -> Arg {
        Arg::with_name("offline")
            .long("offline")
            .help("Run in offline mode; nothing will be retrieved.")
    }

    pub fn vcs() -> Arg {
        Arg::with_name("vcs")
            .long("vcs")
            .takes_value(true)
            .possible_values(&["none", "git"])
            .default_value("git")
            .help("The VCS template to use when initializing a new repo")
    }
}
