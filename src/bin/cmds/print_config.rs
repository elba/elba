use clap::{App, ArgMatches, SubCommand};
use elba::util::{config::Config, error::Result};
use toml;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("print-config").about("Prints elba's configuration")
}

pub fn exec(c: &mut Config, _args: &ArgMatches) -> Result<String> {
    println!("{}", toml::Value::try_from(&c).unwrap());

    Ok("".to_string())
}
