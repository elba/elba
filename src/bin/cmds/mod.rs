mod add;
mod build;
mod check;
mod clean;
mod doc;
mod init;
mod install;
mod new;
mod package;
mod print_config;
mod repl;
mod script;
mod search;
mod test;
mod uninstall;
mod update;

use clap::{App, ArgMatches};
use elba::util::{
    config::{Backend, Config},
    error::Result,
    shell::Verbosity,
};
use failure::{format_err, ResultExt};
use itertools::Itertools;
use slog::{o, Discard, Logger};
use slog_async;
use slog_term;
use std::{env, process::Command};

pub type Exec = fn(&mut Config, &ArgMatches) -> Result<String>;

pub fn subcommands() -> Vec<App<'static, 'static>> {
    vec![
        add::cli(),
        build::cli(),
        check::cli(),
        clean::cli(),
        doc::cli(),
        init::cli(),
        install::cli(),
        new::cli(),
        package::cli(),
        print_config::cli(),
        repl::cli(),
        script::cli(),
        search::cli(),
        test::cli(),
        uninstall::cli(),
        update::cli(),
    ]
}

pub fn execute_internal(cmd: &str) -> Option<Exec> {
    match cmd {
        "add" => Some(add::exec),
        "build" => Some(build::exec),
        "check" => Some(check::exec),
        "clean" => Some(clean::exec),
        "doc" => Some(doc::exec),
        "init" => Some(init::exec),
        "install" => Some(install::exec),
        "new" => Some(new::exec),
        "package" => Some(package::exec),
        "print-config" => Some(print_config::exec),
        "repl" => Some(repl::exec),
        "script" => Some(script::exec),
        "search" => Some(search::exec),
        "test" => Some(test::exec),
        "uninstall" => Some(uninstall::exec),
        "update" => Some(update::exec),
        _ => None,
    }
}

pub fn execute_external(cmd: &str, args: &ArgMatches) -> Result<String> {
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
        })?
        .wait_with_output()
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

mod get {
    use super::*;
    use elba::cli::build::BuildCtx;
    use slog::Drain;

    pub fn build_ctx(c: &mut Config, args: &ArgMatches) -> BuildCtx {
        let logger = get::logger(c, args);

        BuildCtx {
            compiler: c.compiler.clone(),
            indices: c.indices.to_owned(),
            global_cache: c.layout(),
            logger,
            threads: get::threads(c, args),
            shell: c.shell(),
            offline: args.is_present("offline"),
            opts: get::idris_opts(c, args),
        }
    }

    pub fn logger(c: &mut Config, args: &ArgMatches) -> Logger {
        if args.is_present("debug-log") {
            c.term.verbosity = Verbosity::None;
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::CompactFormat::new(decorator).build().fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            Logger::root(drain, o!())
        } else {
            Logger::root(Discard, o!())
        }
    }

    pub fn backends(c: &mut Config, args: &ArgMatches) -> Backend {
        let mut backend = args
            .value_of_lossy("backend")
            .and_then(|x| {
                let x = x.into_owned();
                c.get_backend(&x)
            })
            .unwrap_or_else(|| c.default_backend());

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

    pub fn threads(_c: &mut Config, args: &ArgMatches) -> u32 {
        args.value_of("threads")
            .and_then(|s| s.parse().ok())
            .unwrap_or(2)
    }

    pub fn idris_opts(_c: &mut Config, args: &ArgMatches) -> Vec<String> {
        let mut res = vec![];

        if let Ok(val) = env::var("IDRIS_OPTS") {
            res.extend(val.split(' ').map(|x| x.to_string()));
        }

        if let Some(vals) = args.values_of("idris-opts") {
            res.extend(vals.map(|x| x.to_string()));
        }

        res
    }
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
        Arg::with_name("threads")
            .long("threads")
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

    pub fn debug_log() -> Arg {
        Arg::with_name("debug-log")
            .long("debug-log")
            .help("Print debug logs instead of prettified output")
    }

    pub fn idris_opts() -> Arg {
        Arg::with_name("idris-opts")
            .multiple(true)
            .last(true)
            .min_values(0)
    }

    pub fn no_verify() -> Arg {
        Arg::with_name("no-verify")
            .long("no-verify")
            .help("Skip building the package to test for validity")
    }
}
