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
use elba::util::{config::Config, errors::Res};
use failure::Error;
use slog::{Discard, Logger};

pub type Exec = fn(&mut Config, &ArgMatches) -> Res<()>;

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

pub fn execute_external(cmd: &str, args: &ArgMatches) -> Result<(), Error> {
    let ext_args: Vec<&str> = args
        .values_of("")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);
    println!("we're supposed to execute elba-{} {:?}", cmd, ext_args);
    Ok(())
}

pub fn logger(_c: &mut Config) -> Logger {
    Logger::root(Discard, o!())
}

pub fn match_backends(c: &mut Config, args: &ArgMatches) -> (bool, String, Vec<String>) {
    (
        args.is_present("portable-backend"),
        if args.is_present("backend") {
            args.value_of_lossy("backend").unwrap().into_owned()
        } else if args.is_present("portable-backend") {
            args.value_of_lossy("portable-backend")
                .unwrap()
                .into_owned()
        } else {
            c.default_codegen.to_owned()
        },
        args.values_of_lossy("cg-opts").unwrap_or_else(|| vec![]),
    )
}

mod args {
    use clap;

    type Arg = clap::Arg<'static, 'static>;

    pub fn target_lib() -> Arg {
        Arg::with_name("lib")
            .long("lib")
            .help("Builds the library target")
    }

    pub fn target_bin() -> Arg {
        Arg::with_name("bin")
            .long("bin")
            .takes_value(true)
            .min_values(0)
            .help("Builds the binaries specified (or all if no argument is provided)")
    }

    pub fn target_test() -> Arg {
        Arg::with_name("test")
            .long("test")
            .takes_value(true)
            .min_values(0)
            .help("Builds the tests specified (or all if no argument is provided)")
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
