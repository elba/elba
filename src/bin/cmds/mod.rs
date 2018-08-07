mod build;
mod clean;
mod init;
mod install;
mod lock;
mod new;
mod repl;
mod test;
mod uninstall;

use clap::{App, ArgMatches};
use elba::build::context::BuildBackend;
use elba::util::{config::Config, errors::Res};
use failure::{Error, ResultExt};
use itertools::Itertools;
use slog::{Discard, Logger};
use std::process::Command;

pub type Exec = fn(&mut Config, &ArgMatches) -> Res<String>;

pub fn subcommands() -> Vec<App<'static, 'static>> {
    vec![
        build::cli(),
        clean::cli(),
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

pub fn match_backends(c: &mut Config, args: &ArgMatches) -> BuildBackend {
    let name = if args.is_present("backend") {
        args.value_of_lossy("backend").unwrap().into_owned()
    } else if args.is_present("portable-backend") {
        args.value_of_lossy("portable-backend")
            .unwrap()
            .into_owned()
    } else {
        c.default_codegen.name.to_owned()
    };

    let portable = if args.is_present("backend") {
        false
    } else if args.is_present("portable-backend") {
        true
    } else {
        c.default_codegen.portable
    };

    BuildBackend {
        portable,
        runner: c.codegen.get(&name).map(|x| x.runner.clone()),
        name,
        opts: args.values_of_lossy("cg-opts").unwrap_or_else(|| vec![]),
    }
}

pub fn match_threads(c: &mut Config, args: &ArgMatches) -> u8 {
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
            Arg::with_name("codegen")
                .long("codegen")
                .conflicts_with("portable-codegen")
                .takes_value(true)
                .number_of_values(1)
                .help("Specifies the codegen backend to use during code generation"),
            Arg::with_name("portable-backend")
                .long("portable-backend")
                .takes_value(true)
                .number_of_values(1)
                .help("Specifies the portable codegen backend to use during code generation"),
            Arg::with_name("cg-opts")
                .long("cg-opts")
                .takes_value(true)
                .min_values(1)
                .help("Options to pass to the codegen backend"),
        ]
    }
}
