use build::{
    context::{BuildConfig, BuildContext, Compiler},
    job::JobQueue,
    Target, Targets,
};
use console::style;
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

pub fn test(ctx: &BuildCtx, project: &Path, backend: &(bool, String, Vec<String>)) -> Res<()> {
    solve_local(ctx, &project, |cache, mut retriever, solve| {
        let mut contents = String::new();
        let mut manifest = fs::File::open(project.join("elba.toml"))
            .context(format_err!("failed to read manifest file"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        if manifest.targets.lib.is_none() {
            bail!("running tests requires a defined library to test")
        }
        if manifest.targets.test.is_empty() {
            bail!("at least one test must be defined")
        }

        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        // TODO: Specifying targets to build
        // By default, we build all test targets.
        let mut root = vec![];
        root.push(Target::Lib);
        for (ix, _) in manifest.targets.test.iter().enumerate() {
            root.push(Target::Test(ix));
        }

        let root = Targets::new(root);

        let ctx = BuildContext {
            backend: &backend,
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
        };

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, root, Some(layout), &ctx)?;
        q.exec(&ctx)?;

        // TODO: Run the tests
        unimplemented!()
    })
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

pub fn build(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, Option<Vec<&str>>, Option<Vec<&str>>),
    backend: &(bool, String, Vec<String>),
) -> Res<()> {
    solve_local(ctx, &project, |cache, mut retriever, solve| {
        let mut contents = String::new();
        let mut manifest = fs::File::open(project.join("elba.toml"))
            .context(format_err!("failed to read manifest file"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        println!("{} Retrieving packages...", style("[2/3]").dim().bold());
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        // By default, we build all lib and bin targets.
        let mut root = vec![];
        if (targets.1.is_none() || targets.0) && manifest.targets.lib.is_some() {
            root.push(Target::Lib);
        } else if targets.0 {
            // The user specifically asked for a lib target but there wasn't any. Error.
            bail!("the package doesn't have a library target. add one before proceeding")
        }

        if targets.1.as_ref().is_some() && manifest.targets.bin.is_empty() {
            // The user specifically asked for a bin target(s) but there wasn't any. Error.
            bail!("the package doesn't have any binary targets. add one before proceeding")
        }

        for (ix, bt) in manifest.targets.bin.iter().enumerate() {
            // Case 1: If the --bin flag is passed by itself, we assume the user wants all binaries.
            //         Or, the --bin flag might come with the name of a binary which we should build.
            let target_specified = targets
                .1
                .as_ref()
                .map(|v| v.is_empty() || v.contains(&bt.name.as_str()))
                .unwrap_or(false);
            // Case 2: Neither --bin nor --lib are specified.
            let neither_specified = !targets.0 && targets.1.is_none();
            if target_specified || neither_specified {
                root.push(Target::Bin(ix));
            }
        }

        // We only build test targets if the user asks for them.
        if let Some(ts) = &targets.2 {
            for (ix, bt) in manifest.targets.test.iter().enumerate() {
                let target_specified = ts.is_empty() || ts.contains(&bt.name.as_str());
                if target_specified {
                    root.push(Target::Test(ix));
                }
            }
        }

        let root = Targets::new(root);

        let ctx = BuildContext {
            backend: &backend,
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
        };

        println!("{} Building targets...", style("[3/3]").dim().bold());

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, root, Some(layout), &ctx)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec(&ctx)
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
    println!("{} Resolving dependencies...", style("[1/3]").dim().bold());
    let solve = solver.solve()?;
    println!("{:>7} Writing lockfile...", style("[inf]").dim());

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
    println!("{} Resolving dependencies...", style("[1/3]").dim().bold());
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
