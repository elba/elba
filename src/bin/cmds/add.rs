use super::get;
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    package::Spec,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::{env::current_dir, str::FromStr};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("add")
        .about("Add a package dependency to the current project")
        .arg(
            Arg::with_name("spec")
                .takes_value(true)
                .required(true)
                .help("The package spec to add"),
        )
        .arg(Arg::with_name("dev").help("Whether to add the package as a dev_dependency"))
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let project = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let spec = Spec::from_str(args.value_of("spec").unwrap())?;

    let bcx = get::build_ctx(c, args);

    let res = build::add(&bcx, &project, &spec, args.is_present("dev"))?;

    Ok(res)
}
