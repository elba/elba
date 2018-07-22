use clap::{App, ArgMatches, SubCommand};
use elba::{cli::build, util::{config::Config, errors::Res}};
use failure::ResultExt;
use std::env::current_dir;
use slog::{Discard, Logger};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("lock")
        .about("Generates an elba.lock according to the manifest.")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let project = current_dir().context(format_err!("couldn't get current dir; doesn't exist or no permissions..."))?;
    let indices = c.indices.iter().cloned().collect::<Vec<_>>();
    let global_cache = c.directories.cache.clone();

    // TODO: Proper log output etc.
    let logger = Logger::root(Discard, o!());

    let ctx = build::BuildCtx {
        project, indices, global_cache, logger
    };

    build::lock(&ctx)?;

    Ok(())
}