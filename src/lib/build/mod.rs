//! Actually building Idris packages.
//!
//! The build process contains two major phases: dependency preparation and target generating.
//!
//! - Dependency preparation:
//!     In this stage, elba retrieves all sources in the resolve tree, and then build each of them
//!     in global cache directory. The works in this phase can be excuted in parallel. When all
//!     dependencies are ready in cache, elba will copy the direct dependency into /target/deps.
//!     Dependency preparation does not necessarily be executed along with target generating and it
//!     could also be used by editors (like rls).
//!
//! - Target generation:
//!     In this stage, Elba builds lib target, binary, docs, benchmarks and tests, only for local package.
//!

pub mod context;
pub mod invoke;
pub mod job;

use build::context::{BuildConfig, BuildContext, CompileMode, Compiler, Layout};
use build::invoke::CompileInvocation;
use build::job::JobQueue;
use failure::ResultExt;
use package::{lockfile::LockfileToml, manifest::Manifest, resolution::IndexRes, Summary};
use petgraph::graph::NodeIndex;
use resolve::Resolver;
use retrieve::{Binary, Cache, Retriever, Source};
use slog::Logger;
use std::{fs, io::prelude::*, path::Path, str::FromStr};
use toml;
use util::{config::Config, errors::Res, graph::Graph, lock::DirLock};

pub fn compile(
    package: &DirLock,
    layout: &Layout,
    config: &Config,
    bc: &BuildConfig,
    logger: &Logger,
) -> Res<()> {
    let mut manifest = fs::File::open(package.path().join("elba.toml"))
        .context(format_err!("failed to read manifest file."))?;
    let mut contents = String::new();
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents).context(format_err!("invalid manifest format"))?;

    // TODO: Get indices from config & cache.
    let cache = Cache::from_disk(&logger, config.directories.cache.clone());
    let compiler = Compiler::new();
    let bcx = BuildContext {
        cache: &cache,
        compiler,
    };

    let solve = solve(&package.path(), &manifest, &config, &cache, logger)?;

    let root = solve[NodeIndex::new(0)].clone();

    let job_queue = JobQueue::new(solve, &bcx)?;
    let deps = job_queue.exec(&bcx)?;

    // TODO: seperate the following into second stage(?)
    match bc.compile_mode {
        CompileMode::Lib => compile_lib(&root, &deps, &layout, &bcx)?,
        CompileMode::Bin => unimplemented!(),
        CompileMode::Doc => unimplemented!(),
    }

    Ok(())
}

fn solve(
    package: &Path,
    manifest: &Manifest,
    config: &Config,
    cache: &Cache,
    logger: &Logger,
) -> Res<Graph<Source>> {
    let def_index = config
        .indices
        .get(0)
        .map(|index| index.clone().into())
        .unwrap_or_else(|| IndexRes::from_str("index+dir+none").unwrap());

    let op = || -> Res<Graph<Summary>> {
        let mut f = fs::File::open(&package.join("elba.lock"))?;
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

    let deps = manifest
        .deps(&def_index, true)
        .into_iter()
        .collect::<Vec<_>>();

    let indices = cache.get_indices(&config.indices);

    let mut retriever = Retriever::new(&cache.logger, &cache, root, deps, indices, lock, def_index);
    let solve = Resolver::new(&retriever.logger.clone(), &mut retriever).solve()?;

    let mut lockfile = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&package.join("elba.lock"))
        .context(format_err!("could not open elba.lock for writing"))?;

    let lf_contents: LockfileToml = solve.clone().into();
    let lf_contents = toml::to_string_pretty(&lf_contents)?;

    lockfile
        .write_all(lf_contents.as_bytes())
        .context(format_err!("could not write to elba.lock"))?;

    // TODO: (important) borrowcheck keeps complaining the code beneath
    // let sources = retriever.retrieve_packages(&solve)?;
    let sources = unimplemented!();

    Ok(sources)
}

fn compile_lib(
    source: &Source,
    deps: &Vec<Binary>,
    layout: &Layout,
    bcx: &BuildContext,
) -> Res<()> {
    let lib_target = source.meta().targets.lib.clone().ok_or_else(|| {
        format_err!(
            "package {} does not contain lib target",
            source.meta().package.name
        )
    })?;

    let targets = lib_target
        .mods
        .iter()
        .map(|mod_name| lib_target.path.join(mod_name.replace(".", "/")))
        .collect();

    let invocation = CompileInvocation {
        src: &source.path().join("src"),
        deps,
        targets: &targets,
        layout: &layout,
    };

    invocation.execute(bcx)?;

    for target in targets {
        let target_bin = target.with_extension("ibc");
        let from = layout.build.join(&target_bin);
        let to = layout.lib.join(&target_bin);
        fs::create_dir_all(to.parent().unwrap())?;
        fs::rename(from, to)?;
    }

    Ok(())
}
