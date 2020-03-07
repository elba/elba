use super::args;
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::new,
    package::Name,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::{env::current_dir, str::FromStr};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("new")
        .about("Creates a new empty project")
        .arg(Arg::with_name("name").required(true))
        .arg(args::vcs())
        .arg(
            Arg::with_name("lib")
                .long("lib")
                .help("Create a library project")
                .multiple(false),
        )
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let name = &*args.value_of_lossy("name").unwrap();
    let name = Name::from_str(name)
        .with_context(|e| format_err!("the name `{}` is invalid: {}", name, e))?;
    let bin = !args.is_present("lib");
    let author = if let Some(profile) = &c.profile {
        Some((profile.name.clone(), profile.email.clone()))
    } else {
        None
    };
    let cdir = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let path = cdir.join(name.name().to_string());

    let new_ctx = new::NewCtx {
        path,
        author,
        name,
        bin,
        git: args.value_of_lossy("vcs").unwrap() == "git",
    };

    new::new(new_ctx)
}
