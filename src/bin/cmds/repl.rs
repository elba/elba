use super::{args, logger, match_backends, match_threads};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("repl")
        .about("Launches a repl, loading the root package")
        .arg(args::build_threads())
        .arg(args::target_bin())
        .arg(args::target_lib())
        .args(&args::backends())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let indices = c.indices.to_vec();
    let global_cache = c.layout();
    let logger = logger(c);
    let threads = match_threads(c, args);
    let backend = match_backends(c, args);
    let shell = c.shell();

    let ts = (
        args.is_present("lib"),
        args.values_of("bin").map(|x| x.collect::<Vec<_>>()),
    );

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
        threads,
        shell,
    };

    build::repl(&ctx, &project, &ts, &backend)
}
