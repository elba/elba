#[macro_use]
extern crate clap;

mod cmds;

use clap::{App, AppSettings, Arg, ArgMatches};
use console::style;
use elba::util::{config::Config, shell::Verbosity};
use failure::{Error, ResultExt};
use std::{process::exit, time::Instant};

// TODO: Tasks and scripts (i.e. hooks)
// Tasks are binary dependencies which can be executed from within the project with `elba task`.
// The target directory would have a directory to store the binaries (or they could be symlinks
// to the global dir, and the bin would be stored in the global dir)
//
// Rather than having a separate [tasks] section, we could just combine it in the dependencies
// section.
//
//
//
// Scripts subsume hooks, and are arbitrary shell commands. They function basically like npm
// scripts. Certain special scripts are run automatically by elba after certain actions
// (prebuild: run after building deps and before building this package, preinstall: after
// building this package and before). Otherwise, arbitrary scripts can be run with
// `elba script`.
//
// elba should have a built-in script for running an Idris file (we can simulate Cargo build
// scripts this way)
//
// We can't just delegate this to the equivalent of `cargo-make` (i.e. a task-running plugin)
// because elba won't use it when building dependencies - elba will only ever call `elba build`,
// because it won't know about your task-running thing unless you can tell it to. In fact, scripts
// would basically subsume task-running plugins anyway.

fn cli() -> App<'static, 'static> {
    App::new("elba")
        .about("A package manager for the Idris programming language")
        .setting(AppSettings::AllowExternalSubcommands)
        .version(crate_version!())
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Verbose output")
                .global(true)
                .conflicts_with("quiet"),
        )
        .arg(
            Arg::with_name("quiet")
                .long("quiet")
                .help("Quiet output")
                .global(true),
        )
        .arg(
            Arg::with_name("color")
                .long("color")
                .help("Force-enable color output")
                .global(true)
                .conflicts_with("no-color"),
        )
        .arg(
            Arg::with_name("no-color")
                .long("no-color")
                .help("Disable color output")
                .global(true),
        )
        .subcommands(cmds::subcommands())
}

fn unalias(c: &Config, cmd: &str) -> Option<String> {
    c.alias.get(cmd).map(|x| x.to_owned())
}

// This code is adapted from Cargo <3
fn expand_aliases(
    config: &mut Config,
    args: ArgMatches<'static>,
) -> Result<ArgMatches<'static>, Error> {
    let shell = config.shell();

    if let (cmd, Some(args)) = args.subcommand() {
        match (cmds::execute_internal(cmd), unalias(config, cmd)) {
            (None, Some(alias)) => {
                let mut vec = vec![alias];
                vec.extend(
                    args.values_of("")
                        .unwrap_or_default()
                        .map(|s| s.to_string()),
                );
                let args = cli()
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(vec)?;
                return expand_aliases(config, args);
            }
            (Some(_), Some(_)) => {
                shell.println_unindented(
                    style("[warn]").yellow().bold(),
                    format!("Builtin command shadows alias {}", cmd),
                    Verbosity::Normal,
                );
            }
            (_, None) => {}
        }
    };
    Ok(args)
}

fn go() -> Result<String, Error> {
    let args = cli().get_matches();
    let mut config =
        Config::new().with_context(|e| format!("could not load configuration:\n{}", e))?;
    let args = expand_aliases(&mut config, args)?;

    if args.is_present("verbose") {
        config.verbosity(Verbosity::Verbose);
    } else if args.is_present("quiet") {
        config.verbosity(Verbosity::Quiet);
    }

    if args.is_present("color") {
        config.color(true);
    } else if args.is_present("no-color") {
        config.color(false);
    }

    let (cmd, subcommand_args) = match args.subcommand() {
        (cmd, Some(args)) => (cmd, args),
        _ => {
            cli().print_help()?;
            return Ok("".to_string());
        }
    };

    if let Some(exec) = cmds::execute_internal(cmd) {
        return exec(&mut config, subcommand_args);
    }

    let res = cmds::execute_external(cmd, subcommand_args);

    if res.is_err() {
        cli().print_help()?;
    }

    res
}

fn main() {
    let start = Instant::now();
    let res = go();

    println!();
    match res {
        Err(e) => {
            eprintln!("{} {}", style("error:").red().bold(), e);
            exit(1);
        }
        Ok(st) => {
            let elapsed = start.elapsed();
            if !st.is_empty() {
                println!(
                    "{} {} [{}.{}s]",
                    style("done!").green().bold(),
                    st,
                    elapsed.as_secs(),
                    elapsed.subsec_millis() / 10
                );
            }
            exit(0);
        }
    }
}
