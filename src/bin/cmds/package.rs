use std::env::current_dir;

use clap::{App, ArgMatches, SubCommand};
use elba::{
    cli::{build, index},
    util::{
        config::{Backend, Config},
        error::Result,
    },
};
use failure::{format_err, ResultExt};

use super::{args, get};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("package")
        .arg(args::no_verify())
        .about("Compiles the package and packages it into a nice tarball")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);
    let (project, _) = build::find_manifest(&project, false, None).unwrap();

    if !args.is_present("no-verify") {
        build::build(
            &ctx,
            &project,
            &(true, false, None, None),
            true,
            &Backend::default(),
        )?;
    }

    let (gz_name, _) = index::package(&project)?;

    Ok(format!(
        "created compressed tarball at `{}`",
        gz_name.display()
    ))
}
