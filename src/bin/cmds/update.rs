use super::{args, match_logger, match_threads, match_idris_opts};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::build,
    package::Spec,
    util::{config::Config, errors::Res},
};
use failure::ResultExt;
use std::{env::current_dir, str::FromStr};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("update")
        .arg(args::debug_log())
        .arg(args::idris_opts())
        .arg(
            Arg::with_name("dependencies")
                .multiple(true)
                .help("The dependencies of the package to update (default is all packages)"),
        ).about("Generates or updates elba.lock according to the manifest")
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
        threads: match_threads(c, args),
        shell: c.shell(),
        offline: args.is_present("offline"),
        opts: match_idris_opts(c, args),
    };

    let packages = args
        .values_of("update")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![])
        .into_iter()
        .map(|spec| {
            Spec::from_str(spec)
                .with_context(|e| format_err!("the spec `{}` is invalid:\n{}", spec, e))
        }).collect::<Result<Vec<_>, _>>()?;

    build::update(&ctx, &project, Some(&packages))
}
