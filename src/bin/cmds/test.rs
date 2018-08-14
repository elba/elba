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
            Arg::with_name("test-threads")
                .long("test-threads")
                .takes_value(true)
                .number_of_values(1)
                .help("The number of threads to use to simultaneously run test binaries"),
        ).arg(
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
    let shell = c.shell();

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
        threads,
        shell,
    };

    // This is where our default codegen backend is set
    let backend = match_backends(c, args);

    let targets = args
        .values_of("target")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);

    let test_threads = args
        .value_of("test-threads")
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or(1);

    build::test(&ctx, &project, &targets, &backend, test_threads)
}
