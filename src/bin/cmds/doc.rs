use super::{args, get};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("doc")
        .about("Builds the docs for the root package")
        .arg(args::build_threads())
        .arg(args::debug_log())
        .arg(args::offline())
        .arg(args::idris_opts())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);

    build::doc(&ctx, &project)
}
