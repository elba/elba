mod init;
mod lock;
mod new;

use clap::{App, ArgMatches};
use elba::util::{config::Config, errors::Res};
use failure::Error;

pub type Exec = fn(&mut Config, &ArgMatches) -> Res<()>;

pub fn subcommands() -> Vec<App<'static, 'static>> {
    vec![new::cli(), init::cli(), lock::cli()]
}

pub fn execute_internal(cmd: &str) -> Option<Exec> {
    match cmd {
        "new" => Some(new::exec),
        "init" => Some(init::exec),
        "lock" => Some(lock::exec),
        _ => None,
    }
}

pub fn execute_external(cmd: &str, args: &ArgMatches) -> Result<(), Error> {
    let ext_args: Vec<&str> = args
        .values_of("")
        .map(|x| x.collect())
        .unwrap_or_else(|| vec![]);
    println!("we're supposed to execute elba-{} {:?}", cmd, ext_args);
    Ok(())
}
