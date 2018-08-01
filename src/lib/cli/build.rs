use build::{
    context::{BuildConfig, BuildContext, Compiler},
    job::JobQueue,
    Target, Targets
};
use failure::ResultExt;
use package::{
    lockfile::LockfileToml,
    manifest::Manifest,
    resolution::{DirectRes, IndexRes},
    Name, PackageId, Summary,
};
use resolve::Resolver;
use retrieve::cache::Cache;
use retrieve::cache::OutputLayout;
use retrieve::Retriever;
use slog::Logger;
use std::{
    fs,
    io::prelude::*,
    path::{Path, PathBuf},
    str::FromStr,
};
use toml;
use util::{errors::Res, graph::Graph, lock::DirLock};

pub struct BuildCtx {
    pub indices: Vec<DirectRes>,
    pub global_cache: PathBuf,
    pub logger: Logger,
}

// TODO: Each of these functions returns a closure that can be passed into solve_local or solve_remote
// bench and test are just building executables and running them
pub fn bench(ctx: &BuildCtx, project: &Path) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

pub fn test(ctx: &BuildCtx, project: &Path) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

// The name argument is a Result because we want a generic Either type, but that's not in std
// and I don't feel like making a new enum just for this
// Also the Err variant is a PathBuf because I couldn't get it to take &Path without ownership
// problems in the bin code.
pub fn install(ctx: &BuildCtx, name: Result<Name, PathBuf>) -> Res<()> {
    match name {
        Ok(name) => solve_remote(ctx, name, |_, _, _| unimplemented!()),
        Err(path) => solve_local(ctx, &path, |_, _, _| unimplemented!()),
    }
}

pub fn uninstall(ctx: &BuildCtx, name: Name) -> Res<()> {
    unimplemented!()
}

pub fn repl(ctx: &BuildCtx, project: &Path) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| unimplemented!())
}

pub fn build(ctx: &BuildCtx, project: &Path) -> Res<()> {
    solve_local(ctx, &project, |cache, mut retriever, solve| {
        let mut contents = String::new();
        let mut manifest = fs::File::open(project.join("elba.toml"))
            .context(format_err!("failed to read manifest file"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        // TODO: Specifying targets to build
        // By default, we build all lib and bin targets.
        let mut root = vec![];
        if manifest.targets.lib.is_some() {
            root.push(Target::Lib);
        }
        
        for (ix, _) in manifest.targets.bin.iter().enumerate() {
            root.push(Target::Bin(ix));
        }
        
        let root = Targets::new(root);

        let ctx = BuildContext {
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
        };

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, root, &ctx)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec(&ctx, &Some(layout))
    })
}

pub fn lock(ctx: &BuildCtx, project: &Path) -> Res<()> {
    solve_local(ctx, &project, |_, _, _| Ok(()))
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

    let cache = Cache::from_disk(&ctx.logger, &ctx.global_cache)?;
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
    let cache = Cache::from_disk(&ctx.logger, &ctx.global_cache)?;
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
