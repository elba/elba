use super::{args, logger, match_backends};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("test")
        .about("Runs the tests of the root package")
        .args(&args::backends())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.to_vec();
    let global_cache = c.directories.cache.clone();

    let logger = logger(c);

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
    };

    // This is where our default codegen backend is set
    let backend = match_backends(c, args);

    build::test(&ctx, &project, &backend)
}
