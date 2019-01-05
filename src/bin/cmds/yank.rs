use super::{args, get};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::backend,
    package::Spec,
    util::{config::Config, errors::Res},
};
use failure::format_err;
use std::str::FromStr;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("yank")
        .about("Remove a pushed crate from the index")
        .arg(
            Arg::with_name("package")
                .takes_value(true)
                .help("The package spec to yank (name:version)"),
        )
        .arg(args::index())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let bck = get::index(c, args)?;
    let spec = Spec::from_str(args.value_of("package").unwrap())?;

    if spec.version.is_none() || spec.resolution.is_some() {
        return Err(format_err!(
            "package spec must be in the format name:version"
        ));
    }

    let bcx = get::build_ctx(c, args);
    let bck_text = bck.to_string();
    let ctx = backend::BackendCtx {
        index: bck,
        data_dir: c.directories.data.clone(),
    };

    backend::yank(&bcx, &ctx, &spec.name, spec.version.as_ref().unwrap())?;

    Ok(format!("yanked package {} from index {}", spec, bck_text))
}
