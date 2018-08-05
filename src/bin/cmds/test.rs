use super::{args, logger, match_backends, match_threads};
use clap::{App, Arg, ArgMatches, SubCommand};
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
        .arg(args::build_threads())
        .arg(
            Arg::with_name("target")
                .multiple(true)
                .help("The names of the tests to run (all tests are run if unspecified)"),
        )
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.to_vec();
    let global_cache = c.directories.cache.clone();
    let logger = logger(c);
    let threads = match_threads(c, args);

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
        threads,
    };

    // This is where our default codegen backend is set
    let backend = match_backends(c, args);

    let targets = args
        .values_of("target")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);

    build::test(&ctx, &project, &targets, &backend)
}
