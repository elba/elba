use super::{args, get};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("check")
        .about("Checks the root package without generating binaries")
        .arg(args::target_lib())
        .arg(args::target_bin())
        .arg(args::target_test())
        .arg(args::build_threads())
        .arg(args::offline())
        .arg(args::debug_log())
        .arg(args::idris_opts())
        .args(&args::backends())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);

    let ts = (
        args.is_present("lib"),
        false,
        args.values_of("bin").map(|x| x.collect::<Vec<_>>()),
        args.values_of("test").map(|x| x.collect::<Vec<_>>()),
    );

    // This is where our default codegen backend is set
    let backend = get::backends(c, args);

    build::build(&ctx, &project, &ts, false, &backend)
}
