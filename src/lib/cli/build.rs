use build::{
    context::{BuildBackend, BuildConfig, BuildContext, Compiler},
    job::JobQueue,
    Target, Targets,
};
use console::style;
use failure::ResultExt;
use package::{
    lockfile::LockfileToml,
    manifest::Manifest,
    resolution::{DirectRes, IndexRes},
    PackageId, Spec, Summary,
};
use petgraph::graph::NodeIndex;
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
    pub threads: u8,
}

pub fn test(
    ctx: &BuildCtx,
    project: &Path,
    targets: &[&str],
    backend: &BuildBackend,
) -> Res<String> {
    solve_local(ctx, &project, 5, |cache, mut retriever, solve| {
        let mut contents = String::new();
        let mut manifest = fs::File::open(project.join("elba.toml"))
            .context(format_err!("failed to read maifest file (elba.toml)"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        if manifest.targets.lib.is_none() {
            bail!("running tests requires a defined library to test")
        }
        if manifest.targets.test.is_empty() {
            bail!("at least one test must be defined")
        }

        println!("{} Retrieving packages...", style("[3/5]").dim().bold());
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let mut root = vec![];
        root.push(Target::Lib);
        let emp = targets.is_empty();
        for (ix, bt) in manifest.targets.test.iter().enumerate() {
            if emp || targets.contains(&bt.name.as_str()) {
                root.push(Target::Test(ix));
            }
        }

        let root = Targets::new(root);
        let ctx = BuildContext {
            backend,
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        println!("{} Building targets...", style("[4/5]").dim().bold());

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, root, Some(layout), &ctx)?;
        q.exec(&ctx)?;

        println!("{} Running tests...", style("[5/5]").dim().bold());

        // TODO: Run the tests
        unimplemented!()
    })
}

// The name argument is a Result because we want a generic Either type, but that's not in std
// and I don't feel like making a new enum just for this
// Also the Err variant is a PathBuf because I couldn't get it to take &Path without ownership
// problems in the bin code.
pub fn install(
    ctx: &BuildCtx,
    name: Result<Spec, PathBuf>,
    targets: &[&str],
    backend: &BuildBackend,
    force: bool,
) -> Res<String> {
    let f = |cache: &Cache, mut retriever: Retriever, solve| -> Res<String> {
        println!("{} Retrieving packages...", style("[3/5]").dim().bold());
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let mut contents = String::new();
        let mut manifest = fs::File::open(sources[NodeIndex::new(0)].path().join("elba.toml"))
            .context(format_err!("failed to read maifest file (elba.toml)"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        // By default, we build all bin targets.
        let mut root = vec![];
        let emp = targets.is_empty();
        for (ix, bt) in manifest.targets.bin.iter().enumerate() {
            if emp || targets.contains(&bt.name.as_str()) {
                root.push(Target::Test(ix));
            }
        }
        let root = Targets::new(root);

        let ctx = BuildContext {
            backend,
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        println!("{} Building targets...", style("[4/5]").dim().bold());

        // We unconditionally use a global OutputLayout to force rebuilding of root packages
        // and to avoid dealing with making our own for global/remote packages

        let q = JobQueue::new(sources, root, None, &ctx)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        let bins = q.exec(&ctx)?;
        let binc = bins.len();

        println!("{} Installing binaries...", style("[5/5]").dim().bold());
        for (path, sum) in bins {
            println!(
                "{:>7} {}",
                style("[ins]").blue(),
                path.file_name().unwrap().to_string_lossy().as_ref()
            );
            cache.store_bin(&path, &sum, force)?;
        }

        Ok(format!(
            "{} binaries installed into {}",
            binc,
            cache.layout.bin.display()
        ))
    };

    match name {
        Ok(name) => solve_remote(ctx, name, 5, f),
        Err(path) => solve_local(ctx, &path, 5, f),
    }
}

pub fn repl(ctx: &BuildCtx, project: &Path) -> Res<String> {
    solve_local(ctx, &project, 4, |_, _, _| unimplemented!())
}

pub fn build(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, Option<Vec<&str>>, Option<Vec<&str>>),
    backend: &BuildBackend,
) -> Res<String> {
    solve_local(ctx, &project, 4, |cache, mut retriever, solve| {
        let mut contents = String::new();
        let mut manifest = fs::File::open(project.join("elba.toml"))
            .context(format_err!("failed to read maifest file (elba.toml)"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest =
            Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

        println!("{} Retrieving packages...", style("[3/4]").dim().bold());
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
            backend,
            // TODO: pick a better compiler pls
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        println!("{} Building targets...", style("[4/4]").dim().bold());

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, root, Some(layout), &ctx)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec(&ctx)?;

        Ok("build output available at `./target`".to_string())
    })
}

pub fn lock(ctx: &BuildCtx, project: &Path) -> Res<String> {
    solve_local(ctx, &project, 2, |_, _, _| {
        Ok("lockfile created at `./elba.lock`".to_string())
    })
}

pub fn solve_local<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Res<String>>(
    ctx: &BuildCtx,
    project: &Path,
    total: u8,
    mut f: F,
) -> Res<String> {
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read maifest file (elba.toml)"))?;
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
    println!(
        "{} Updating package indices...",
        style(format!("[1/{}]", total)).dim().bold()
    );
    let indices = cache.get_indices(&ctx.indices);
    println!(
        "{:>7} Indices stored at {}",
        style("[inf]").dim(),
        cache.layout.indices.display()
    );

    let mut retriever = Retriever::new(&cache.logger, &cache, root, deps, indices, lock, def_index);
    let solver = Resolver::new(&retriever.logger.clone(), &mut retriever);
    println!(
        "{} Resolving dependencies...",
        style(format!("[2/{}]", total)).dim().bold()
    );
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

pub fn solve_remote<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Res<String>>(
    ctx: &BuildCtx,
    name: Spec,
    total: u8,
    mut f: F,
) -> Res<String> {
    let def_index = def_index(ctx);
    let cache = Cache::from_disk(&ctx.logger, &ctx.global_cache)?;
    println!(
        "{} Updating package indices...",
        style(format!("[1/{}]", total)).dim().bold()
    );
    let mut indices = cache.get_indices(&ctx.indices);
    println!(
        "{:>7} Indices stored at {}",
        style("[inf]").dim(),
        cache.layout.indices.display()
    );
    let root = indices.select_by_spec(name)?;

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
    println!(
        "{} Resolving dependencies...",
        style(format!("[2/{}]", total)).dim().bold()
    );
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
