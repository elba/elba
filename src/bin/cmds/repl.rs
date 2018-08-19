use super::{args, match_backends, match_logger, match_threads};
use clap::{App, Arg, ArgMatches, SubCommand};
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
        .arg(args::offline())
        .arg(args::debug_log())
        .arg(
            Arg::with_name("ide-mode")
                .long("ide-mode")
                .help("Launches the interactive IDE backend instead of a normal REPL"),
        ).args(&args::backends())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let logger = match_logger(c, args);
    let threads = match_threads(c, args);
    let backend = match_backends(c, args);

    let ts = (
        args.is_present("lib"),
        args.values_of("bin").map(|x| x.collect::<Vec<_>>()),
    );

    let ctx = build::BuildCtx {
        indices: c.indices.to_vec(),
        global_cache: c.layout(),
        logger,
        threads,
        shell: c.shell(),
        offline: args.is_present("offline"),
    };

    build::repl(&ctx, &project, &ts, &backend, args.is_present("ide-mode"))
}
