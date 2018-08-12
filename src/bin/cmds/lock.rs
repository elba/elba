use super::logger;
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("lock").about("Generates an elba.lock according to the manifest")
}

pub fn exec(c: &mut Config, _args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.to_vec();
    let global_cache = c.directories.cache.clone();
    let logger = logger(c);
    let threads = 1;

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
        threads,
    };

    build::lock(&ctx, &project)
}
