use super::{args, logger, match_backends, match_threads};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("build")
        .about("Builds the root package")
        .arg(args::target_lib())
        .arg(
            Arg::with_name("lib-cg").long("lib-cg").help(
                "Use export lists from the library to generate code with the codegen backend",
            ),
        ).arg(args::target_bin())
        .arg(args::target_test())
        .arg(args::build_threads())
        .args(&args::backends())
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

    let ts = (
        args.is_present("lib"),
        args.is_present("lib-cg"),
        args.values_of("bin").map(|x| x.collect::<Vec<_>>()),
        args.values_of("test").map(|x| x.collect::<Vec<_>>()),
    );

    // This is where our default codegen backend is set
    let backend = match_backends(c, args);

    build::build(&ctx, &project, &ts, &backend)
}
