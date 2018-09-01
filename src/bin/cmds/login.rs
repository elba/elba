use super::{args, get};
use clap::{App, ArgMatches, SubCommand};
use elba::util::{config::Config, errors::Res};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("login")
        .about("Log in to a given backend")
        .arg(args::index())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let _bck = get::index(c, args);

    unimplemented!()
}
