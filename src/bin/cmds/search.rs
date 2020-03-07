use super::get;
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::index,
    util::{config::Config, error::Result},
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
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let query = args.value_of("query").unwrap();
    let bcx = get::build_ctx(c, args);

    println!("{}", index::search(&bcx, &query)?);

    Ok("search complete".to_string())
}
