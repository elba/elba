use clap::{App, ArgMatches, SubCommand};
use elba::{cli::new, package::Name, util::{config::Config, errors::Res}};
use failure::ResultExt;
use std::env::current_dir;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("lock")
        .about("Generates an elba.lock according to the manifest.")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<()> {
    let name = &*args.value_of_lossy("name").unwrap();
    let name = Name::new(name.to_owned(), name.to_owned());
    let bin = !args.is_present("lib");
    let author = if let Some(profile) = &c.profile {
        Some((profile.name.clone(), profile.email.clone()))
    } else {
        None
    };
    let cdir = current_dir().context(format_err!("couldn't get current dir; doesn't exist or no permissions..."))?;
    let path = cdir.join(format!("{}", name.name()));

    let new_ctx = new::NewCtx {
        path: path,
        author,
        name,
        bin,
    };

    new::new(new_ctx)
}