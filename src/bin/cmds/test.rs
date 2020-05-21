use super::{args, get};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("test")
        .about("Runs the tests of the root package")
        .args(&args::backends())
        .arg(args::build_threads())
        .arg(args::offline())
        .arg(args::debug_log())
        .arg(
            Arg::with_name("test-threads")
                .long("test-threads")
                .takes_value(true)
                .number_of_values(1)
                .help("The number of threads to use to simultaneously run test binaries"),
        )
        .arg(
            Arg::with_name("targets")
                .multiple(true)
                .help("The names of the tests to run (all tests are run if unspecified)"),
        )
        .arg(args::idris_opts())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);

    // This is where our default codegen backend is set
    let backend = get::backends(c, args);

    let targets = args
        .values_of("targets")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);

    let test_threads = args
        .value_of("test-threads")
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or(1);

    build::test(&ctx, &project, &targets, &backend, test_threads)
}
