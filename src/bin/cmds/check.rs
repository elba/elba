use super::logger;
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("check").about("Checks the root package and all dependents for errors")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.iter().cloned().collect::<Vec<_>>();
    let global_cache = c.directories.cache.clone();

    let logger = logger(c);

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
    };

    build::solve_local(&ctx, project)?;

    // TODO: Do more stuff
    unimplemented!()
}
