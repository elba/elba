use failure::ResultExt;
use package::{
    lockfile::LockfileToml,
    manifest::Manifest,
    resolution::{DirectRes, IndexRes},
    Name, PackageId, Summary,
};
use resolve::Resolver;
use retrieve::cache::Cache;
use retrieve::Retriever;
use slog::Logger;
use std::{fs, io::prelude::*, path::{Path, PathBuf}, str::FromStr};
use toml;
use util::errors::Res;
use util::graph::Graph;

pub struct BuildCtx {
    pub indices: Vec<DirectRes>,
    pub global_cache: PathBuf,
    pub logger: Logger,
}

// TODO: Maybe each one of these should return a closure that we can pass into solve_*
pub fn build(ctx: &BuildCtx, project: PathBuf) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

pub fn check(ctx: &BuildCtx, project: PathBuf) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

pub fn install(ctx: &BuildCtx, name: Name) -> Res<()> {
    solve_remote(ctx, name, |_, _, _| unimplemented!())
}

pub fn lock(ctx: &BuildCtx, project: PathBuf) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| Ok(()))
}

pub fn repl(ctx: &BuildCtx, project: PathBuf) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

pub fn solve_local<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Res<()>>(
    ctx: &BuildCtx,
    project: &Path,
    mut f: F,
) -> Res<()> {
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file"))?;
    let mut contents = String::new();
    manifest.read_to_string(&mut contents)?;

    let manifest = Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

    let op = || -> Res<Graph<Summary>> {
        let mut f = fs::File::open(&project.join("elba.lock"))?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        let toml = LockfileToml::from_str(&contents)?;

        Ok(toml.into())
    };

    let lock = if let Ok(solve) = op() {
        solve
    } else {
        Graph::default()
    };

    let root = manifest.summary();

    let def_index = def_index(ctx);

    let deps = manifest
        .deps(&def_index, true)
        .into_iter()
        .collect::<Vec<_>>();

    let cache = Cache::from_disk(&ctx.logger, &ctx.global_cache);
    let indices = cache.get_indices(&ctx.indices);

    let mut retriever = Retriever::new(&cache.logger, &cache, root, deps, indices, lock, def_index);
    let solver = Resolver::new(&retriever.logger.clone(), &mut retriever);
    let solve = solver.solve()?;

    let mut lockfile = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&project.join("elba.lock"))
        .context(format_err!("could not open elba.lock for writing"))?;

    let lf_contents: LockfileToml = solve.clone().into();
    let lf_contents = toml::to_string_pretty(&lf_contents)?;

    lockfile
        .write_all(lf_contents.as_bytes())
        .context(format_err!("could not write to elba.lock"))?;

    f(&cache, retriever, solve)
}

pub fn solve_remote<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Res<()>>(
    ctx: &BuildCtx,
    name: Name,
    mut f: F,
) -> Res<()> {
    let def_index = def_index(ctx);
    let cache = Cache::from_disk(&ctx.logger, &ctx.global_cache);
    let mut indices = cache.get_indices(&ctx.indices);
    let root = indices.select_by_name(name)?;

    let deps = indices
        .select(&root)
        .unwrap()
        .dependencies
        .iter()
        .cloned()
        .map(|d| (PackageId::new(d.name, d.index.into()), d.req))
        .collect::<Vec<_>>();

    let lock = Graph::default();

    let mut retriever = Retriever::new(&cache.logger, &cache, root, deps, indices, lock, def_index);
    let solve = Resolver::new(&retriever.logger.clone(), &mut retriever).solve()?;

    f(&cache, retriever, solve)
}

fn def_index(ctx: &BuildCtx) -> IndexRes {
    if ctx.indices.is_empty() {
        IndexRes::from_str("index+dir+none").unwrap()
    } else {
        ctx.indices[0].clone().into()
    }
}
