// TODO: remove this once we start actually developing the CLI tool
#![allow(unused_variables, dead_code)]

#[macro_use]
extern crate clap;
extern crate elba;
extern crate failure;
extern crate toml;

mod cmds;

use clap::{App, AppSettings, Arg, ArgMatches};
use elba::util::config::Config;
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
                .help("Verbose output (-vv = Very Verbose)")
                .multiple(true)
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

fn main() -> Result<(), Error> {
    let args = cli().get_matches();
    // TODO: Actually get correct config
    let mut config = Config::default();
    let args = expand_aliases(&mut config, args)?;

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
