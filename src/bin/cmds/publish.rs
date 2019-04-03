use super::{args, get};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::registry,
    util::{config::Config, errors::Res},
};
use failure::{format_err, ResultExt};
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("publish")
        .about("Publishes a package to a registry")
        .arg(args::index())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let bck = get::index(c, args)?;
    let bck_text = bck.to_string();
    let bcx = get::build_ctx(c, args);
    let ctx = registry::RegistryCtx {
        index: bck,
        data_dir: c.directories.data.clone(),
    };

    // TODO: We require verification of packages before publishing; maybe not?
    registry::publish(&bcx, &ctx, &project, true)?;

    Ok(format!("published package to index {}", bck_text))
}
