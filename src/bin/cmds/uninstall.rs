use super::logger;
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    package::Name,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::str::FromStr;

// TODO: Uninstall all bins of a package or individual bin?
pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("uninstall")
        .about("Uninstalls the binaries of a package")
        .arg(Arg::with_name("name").required(true))
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let name = &*args.value_of_lossy("name").unwrap();
    let name = Name::from_str(name).context(format_err!("the name `{}` is invalid.", name))?;

    let logger = logger(c);
    let indices = c.indices.to_vec();
    let global_cache = c.directories.cache.clone();

    let ctx = build::BuildCtx {
        indices,
        global_cache,
        logger,
    };

    build::uninstall(&ctx, name)
}
