#[macro_use]
extern crate clap;
extern crate console;
extern crate elba;
#[macro_use]
extern crate failure;
// extern crate indicatif;
extern crate itertools;
extern crate toml;
#[macro_use]
extern crate slog;
extern crate slog_term;

mod cmds;

use clap::{App, AppSettings, Arg, ArgMatches};
use console::style;
use elba::util::{config::Config, shell::Verbosity};
use failure::{Error, ResultExt};
use std::{process::exit, time::Instant};

// Interaction with the main repo would just be implemented as a custom task.
// Maybe tasks should be allowed to be designated in the manifest too. These would be placed in the
// local target bin directory, not the global bin directory, but would otherwise be treated like
// dependencies.

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
        ).arg(
            Arg::with_name("quiet")
                .long("quiet")
                .help("Quiet output")
                .global(true),
        ).arg(
            Arg::with_name("color")
                .long("color")
                .help("Force-enable color output")
                .global(true)
                .conflicts_with("no-color"),
        ).arg(
            Arg::with_name("no-color")
                .long("no-color")
                .help("Disable color output")
                .global(true),
        ).subcommands(cmds::subcommands())
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
