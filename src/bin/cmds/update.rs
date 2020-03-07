use super::{args, get};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    package::Spec,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::{env::current_dir, str::FromStr};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("update")
        .arg(args::debug_log())
        .arg(
            Arg::with_name("dependencies")
                .multiple(true)
                .help("The dependencies of the package to update (default is all packages)"),
        )
        .about("Generates or updates elba.lock according to the manifest")
        .arg(args::idris_opts())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let ctx = get::build_ctx(c, args);

    let packages = args
        .values_of("update")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![])
        .into_iter()
        .map(|spec| {
            Spec::from_str(spec)
                .with_context(|e| format_err!("the spec `{}` is invalid:\n{}", spec, e))
                .map_err(Into::into)
        })
        .collect::<Result<Vec<_>>>()?;

    build::update(&ctx, &project, Some(&packages))
}
