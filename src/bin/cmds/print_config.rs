use clap::{App, ArgMatches, SubCommand};
use elba::util::{config::Config, errors::Res};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("print-config").about("Prints elba's configuration")
}

pub fn exec(c: &mut Config, _args: &ArgMatches) -> Res<String> {
    println!("{:#?}", c);

    Ok("".to_string())
}
