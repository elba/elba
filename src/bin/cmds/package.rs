use super::{args, get};
use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::backend,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("package")
        .arg(args::no_verify())
        .about("Compiles the package and packages it into a nice tarball")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);

    let (gz_name, _, _) = backend::package(&ctx, &project, !args.is_present("no-verify"))?;

    Ok(format!(
        "created compressed tarball at `{}`",
        gz_name.display()
    ))
}
