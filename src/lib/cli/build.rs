use failure::ResultExt;
use package::{
    lockfile::LockfileToml,
    manifest::Manifest,
    resolution::{DirectRes, IndexRes},
};
use resolve::{solve::Solve, Resolver};
use retrieve::cache::Cache;
use retrieve::Retriever;
use slog::Logger;
use std::{fs, io::prelude::*, path::PathBuf, str::FromStr};
use toml;
use util::errors::Res;

pub struct BuildCtx {
    pub project: PathBuf,
    pub indices: Vec<DirectRes>,
    pub global_cache: PathBuf,
    pub logger: Logger,
}

pub fn lock(ctx: &BuildCtx) -> Res<(Cache, Solve)> {
    let (cache, solve) = solve(ctx)?;

    let mut lockfile = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&ctx.project.join("elba.lock"))
        .context(format_err!("could not open elba.lock for writing"))?;

    let lf_contents: LockfileToml = solve.clone().into();
    let lf_contents = toml::to_string_pretty(&lf_contents)?;

    lockfile
        .write_all(lf_contents.as_bytes())
        .context(format_err!("could not write to elba.lock"))?;

    Ok((cache, solve))
}

fn solve(ctx: &BuildCtx) -> Res<(Cache, Solve)> {
    let mut manifest = fs::File::open(ctx.project.join("elba.toml"))
        .context(format_err!("failed to read manifest file."))?;
    let mut contents = String::new();
    manifest.read_to_string(&mut contents)?;

    let manifest = Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

    let def_index = if ctx.indices.is_empty() {
        IndexRes::from_str("index+dir+file://none").unwrap()
    } else {
        ctx.indices[0].clone().into()
    };

    // TODO: Get indices from config & cache.
    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), def_index.clone());
    let indices = cache.get_indices(&ctx.indices);

    let op = || -> Res<Solve> {
        let mut f = fs::File::open(&ctx.project.join("elba.lock"))?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        let toml = LockfileToml::from_str(&contents)?;

        Ok(toml.into())
    };

    let lock = if let Ok(solve) = op() {
        solve
    } else {
        Solve::default()
    };

    let root = manifest.summary();

    let deps = manifest
        .deps(&cache.def_index, true)
        .into_iter()
        .collect::<Vec<_>>();

    let mut retriever = Retriever::new(&cache.logger, &cache, root, deps, indices, lock);
    let solve = Resolver::new(&retriever.logger.clone(), &mut retriever).solve()?;

    Ok((cache, solve))
}
