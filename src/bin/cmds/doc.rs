use super::{args, logger, match_threads};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("doc")
        .about("Builds the docs for the root package")
        .arg(args::build_threads())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.to_vec();
    let global_cache = c.layout();
    let logger = logger(c);
    let threads = match_threads(c, args);
    let shell = c.shell();

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
        threads,
        shell,
    };

    build::doc(&ctx, &project)
}
