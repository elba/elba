use super::{args, get};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    package::Spec,
    retrieve::cache::Cache,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::str::FromStr;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("uninstall")
        .about("Uninstalls the binaries of a package")
        .arg(Arg::with_name("spec").required(true))
        .arg(args::target_bin())
        .arg(args::debug_log())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let spec = &*args.value_of_lossy("spec").unwrap();
    let spec = Spec::from_str(spec)
        .with_context(|e| format_err!("the spec `{}` is invalid:\n{}", spec, e))?;

    let targets = args
        .values_of("bin")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);

    let logger = get::logger(c, args);
    let shell = c.shell();

    let cache = Cache::from_disk(&logger, c.layout(), shell)?;

    let rc = cache.remove_bins(&spec, &targets)?;

    Ok(format!("removed {} binaries", rc))
}
