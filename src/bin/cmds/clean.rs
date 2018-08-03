use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::new,
    package::Name,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("clean")
        .about("Cleans the global cache")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let local = !args.is_present("global");

    unimplemented!()
}
