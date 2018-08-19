use super::{args, match_logger};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::build,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("lock")
        .arg(args::debug_log())
        .about("Generates an elba.lock according to the manifest")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let logger = match_logger(c, args);

    let ctx = build::BuildCtx {
        indices: c.indices.to_vec(),
        global_cache: c.layout(),
        logger,
        threads: 1, // irrelevant
        shell: c.shell(),
        offline: args.is_present("offline"),
    };

    build::lock(&ctx, &project)
}
