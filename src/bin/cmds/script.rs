use clap::{App, Arg, ArgMatches, SubCommand};
use console::style;
use elba::{
    build::run_script,
    cli::build::find_manifest_root,
    package::manifest::Manifest,
    util::{config::Config, errors::Res, fmt_multiple, shell::Verbosity},
};
use failure::{format_err, ResultExt};
use std::{env::current_dir, fs, io::Read, str::FromStr};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("script")
        .about("Run a project script")
        .arg(Arg::with_name("script-name").required(true))
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let name = &*args.value_of_lossy("script-name").unwrap();
    let cdir = current_dir().context(format_err!(
        "couldn't get current dir; doesn't exist or no permissions..."
    ))?;

    let mut contents = String::new();
    let project = find_manifest_root(&cdir)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    if let Some(s) = manifest.scripts.get(name) {
        c.shell().println(
            style("Running").dim(),
            format!("script `{}` > {}", name, s),
            Verbosity::Normal,
        );
        c.shell().println_plain(
            fmt_multiple(
                &run_script(&cdir, s)
                    .with_context(|e| format_err!("error running script:\n{}", e))?,
            ),
            Verbosity::Normal,
        );

        Ok(format!("successfully executed script `{}`", name))
    } else {
        Err(format_err!("script `{}` not found", name))
    }
}
