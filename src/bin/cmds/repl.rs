use super::{args, get};
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
        .arg(args::idris_opts())
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

    let ts = (
        args.is_present("lib"),
        args.values_of("bin").map(|x| x.collect::<Vec<_>>()),
    );

    let backend = get::backends(c, args);
    let ctx = get::build_ctx(c, args);

    build::repl(&ctx, &project, &ts, &backend, args.is_present("ide-mode"))
}
