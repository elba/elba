use clap::{App, ArgMatches, SubCommand};
use elba::util::{clear_dir, config::Config, error::Result};
use failure::{format_err, ResultExt};

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("clean").about("Cleans the global cache")
}

pub fn exec(c: &mut Config, _args: &ArgMatches) -> Result<String> {
    let layout = c.layout();

    clear_dir(&layout.src).context(format_err!("couldn't clear {}", layout.src.display()))?;
    clear_dir(&layout.build).context(format_err!("couldn't clear {}", layout.build.display()))?;
    clear_dir(&layout.indices)
        .context(format_err!("couldn't clear {}", layout.indices.display()))?;
    clear_dir(&layout.tmp).context(format_err!("couldn't clear {}", layout.tmp.display()))?;

    Ok("cache directories cleared".to_string())
}
