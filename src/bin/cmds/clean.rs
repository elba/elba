use clap::{App, ArgMatches, SubCommand};
use elba::{
    retrieve::cache::Layout,
    util::{clear_dir, config::Config, errors::Res},
};
use failure::ResultExt;

pub fn cli() -> App<'static, 'static> {
    SubCommand::with_name("clean").about("Cleans the global cache")
}

pub fn exec(c: &mut Config, args: &ArgMatches) -> Res<String> {
    let p = &c.directories.cache;
    if p.exists() {
        let layout = Layout::new(&p)?;

        clear_dir(&layout.src).context(format_err!("couldn't clear {}", layout.src.display()))?;
        clear_dir(&layout.build)
            .context(format_err!("couldn't clear {}", layout.build.display()))?;
        clear_dir(&layout.indices)
            .context(format_err!("couldn't clear {}", layout.indices.display()))?;
        clear_dir(&layout.tmp).context(format_err!("couldn't clear {}", layout.tmp.display()))?;
    }

    Ok(format!("cache directory {} cleared", p.display()))
}
