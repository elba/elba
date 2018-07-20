// TODO: remove this once we start actually developing the CLI tool
#![allow(unused_variables, dead_code)]

#[macro_use]
extern crate clap;
extern crate elba;
#[macro_use]
extern crate failure;
extern crate toml;

mod cmds;

use clap::{App, AppSettings, Arg, ArgMatches};
use elba::util::config::{Config, Verbosity};
use failure::Error;

// Interaction with the main repo would just be implemented as a custom task.
// Maybe tasks should be allowed to be designated in the manifest too. These would be placed in the
// local target bin directory, not the global bin directory, but would otherwise be treated like
// dependencies (or maybe as other root packages).

fn cli() -> App<'static, 'static> {
    App::new("elba")
        .about("A package manager for the Idris programming language")
        .setting(AppSettings::AllowExternalSubcommands)
        .version(crate_version!())
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .help("Verbose output")
                .global(true)
                .conflicts_with("quiet"),
        )
        .arg(
            Arg::with_name("quiet")
                .help("Quiet output")
                .global(true)
        )
        .arg(
            Arg::with_name("color")
                .long("color")
                .help("Force-enable color output")
                .global(true)
                .conflicts_with("no-color")
        )
        .arg(
            Arg::with_name("no-color")
                .long("no-color")
                .help("Disable color output")
                .global(true)
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
                // TODO: A warning here would be nice
            }
            (_, None) => {}
        }
    };
    Ok(args)
}

fn go() -> Result<(), Error> {
    let args = cli().get_matches();
    // TODO: Actually get correct config
    let mut config = Config::default();
    let args = expand_aliases(&mut config, args)?;

    let verbosity = if args.is_present("verbose") {
        Some(Verbosity::Verbose)
    } else if args.is_present("quiet") {
        Some(Verbosity::Quiet)
    } else {
        None
    };

    let color = if args.is_present("color") {
        Some(true)
    } else if args.is_present("no-color") {
        Some(false)
    } else {
        None
    };

    config.configure(verbosity, color);

    let (cmd, subcommand_args) = match args.subcommand() {
        (cmd, Some(args)) => (cmd, args),
        _ => {
            cli().print_help()?;
            return Ok(());
        }
    };

    if let Some(exec) = cmds::execute_internal(cmd) {
        return exec(&mut config, subcommand_args);
    }

    cmds::execute_external(cmd, subcommand_args)
}

// TODO: Actually pretty-print the error, using the `Shell` struct.
// See cargo::exit_with_error and main.rs in the cargo bin.
fn main() {
    let res = go();

    if let Err(e) = res {
        println!("[err] {}", e);
    }
}