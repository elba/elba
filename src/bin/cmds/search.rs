use super::{args, get};
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::registry,
    util::{config::Config, errors::Res},
};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("search")
        .about("Searches for a package in a registry")
        .arg(
            Arg::with_name("query")
                .takes_value(true)
                .required(true)
                .help("The search query."),
        )
        .arg(args::index())
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let bck = get::index(c, args)?;
    let query = args.value_of("query").unwrap();

    let bcx = get::build_ctx(c, args);
    let bck_text = bck.to_string();
    let ctx = registry::RegistryCtx {
        index: bck,
        data_dir: c.directories.data.clone(),
    };

    println!("{}", registry::search(&bcx, &ctx, &query)?);

    Ok(format!("search complete in index {}", bck_text))
}
