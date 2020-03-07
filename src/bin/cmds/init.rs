use super::args;
use clap::{App, Arg, ArgMatches, SubCommand};
use elba::{
    cli::new,
    package::Name,
    util::{config::Config, error::Result},
};
use failure::{format_err, ResultExt};
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("init")
        .about("Initializes an elba project in the current directory")
        .arg(args::vcs())
        .arg(
            Arg::with_name("lib")
                .long("lib")
                .help("Create a library project")
                .multiple(false),
        )
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Result<String> {
    let cdir = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;
    let name = cdir
        .file_name()
        .ok_or_else(|| format_err!("can't make a project in a root directory"))?
        .to_string_lossy()
        .into_owned();
    let name = Name::new(name.clone(), name)?;
    let bin = !args.is_present("lib");
    let author = if let Some(profile) = &c.profile {
        Some((profile.name.clone(), profile.email.clone()))
    } else {
        None
    };

    let new_ctx = new::NewCtx {
        path: cdir,
        author,
        name,
        bin,
        git: args.value_of_lossy("vcs").unwrap() == "git",
    };

    new::init(new_ctx)
}
