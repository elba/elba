use build::{
    context::{BuildConfig, BuildContext, Compiler},
    job::{Job, JobQueue},
    Target, Targets,
};
use console::style;
use crossbeam::queue::MsQueue;
use failure::ResultExt;
use package::{
    lockfile::LockfileToml,
    manifest::Manifest,
    resolution::{DirectRes, IndexRes},
    PackageId, Spec, Summary,
};
use petgraph::graph::NodeIndex;
use resolve::Resolver;
use retrieve::{
    cache::{Cache, Layout, OutputLayout},
    Retriever,
};
use scoped_threadpool::Pool;
use slog::Logger;
use std::{
    env, fs,
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};
use toml;
use util::{
    config::Backend,
    errors::Res,
    fmt_output,
    graph::Graph,
    lock::DirLock,
    shell::{Shell, Verbosity},
};

// TODO: In all commands, pick a better compiler than `Compiler::default()`

pub struct BuildCtx {
    pub indices: Vec<DirectRes>,
    pub global_cache: Layout,
    pub logger: Logger,
    pub threads: u32,
    pub shell: Shell,
    pub offline: bool,
}

pub fn test(
    ctx: &BuildCtx,
    project: &Path,
    targets: &[&str],
    backend: &Backend,
    test_threads: u32,
) -> Res<String> {
    let mut contents = String::new();
    let project = find_manifest_root(project)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    if manifest.targets.test.is_empty() {
        bail!("at least one test must be defined")
    }

    solve_local(ctx, &project, 3, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let bctx = BuildContext {
            backend,
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[2/3]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let bin_dir = layout.bin.clone();

        let mut root = vec![];
        if manifest.targets.lib.is_some() {
            root.push(Target::Lib(false));
        } else {
            ctx.shell.println(
                style("[warn]").yellow().bold(),
                "No lib target for tests to import",
                Verbosity::Normal,
            );
        }
        let emp = targets.is_empty();
        for (ix, bt) in manifest.targets.test.iter().enumerate() {
            if emp || targets.contains(&bt.name.as_str()) {
                root.push(Target::Test(ix));
            }
        }

        let root = Targets::new(root);
        let q = JobQueue::new(sources, &root, Some(layout), &bctx, &ctx.logger, ctx.shell)?;
        q.exec(&bctx)?;

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[3/3]").dim().bold(),
            "Running tests...",
            Verbosity::Quiet,
        );

        let root = root
            .0
            .into_iter()
            .filter_map(|t| {
                if let Target::Test(ix) = t {
                    Some(&manifest.targets.test[ix])
                } else {
                    None
                }
            }).collect::<Vec<_>>();

        // Until pb.println gets added, we can't use progress bars
        // let pb = ProgressBar::new(root.len() as u64);
        // pb.set_style(ProgressStyle::default_bar().template("  [-->] {bar} {pos}/{len}"));

        let results = &MsQueue::new();
        let mut pool = Pool::new(test_threads);

        pool.scoped(|scope| {
            // let mut prg = 0;
            let shell = ctx.shell;
            for test in &root {
                let bin_dir = &bin_dir;
                // let pb = &pb;
                scope.execute(move || {
                    shell.println(style("Running").cyan(), &test.name, Verbosity::Normal);
                    let out = if let Some(r) = &backend.runner {
                        Command::new(r).arg(bin_dir.join(&test.name)).output()
                    } else {
                        Command::new(bin_dir.join(&test.name)).output()
                    };
                    if out.is_err() {
                        shell.println(
                            style("[error]").red().bold(),
                            format!(
                                "Test binary {} could not be executed",
                                bin_dir.join(&test.name).display()
                            ),
                            Verbosity::Quiet,
                        );
                    }
                    results.push(out.map(|x| (&test.name, x)));
                    // prg += 1;
                    // pb.set_position(prg);
                });
            }

            // pb.finish_and_clear();
        });

        ctx.shell.println_empty(Verbosity::Normal);
        let mut errs = 0;
        while let Some(res) = results.try_pop() {
            match res {
                Ok((test, out)) => {
                    ctx.shell.println(
                        if out.status.success() {
                            style("Passed").green()
                        } else {
                            style("Failed").red()
                        },
                        &test,
                        Verbosity::Quiet,
                    );

                    ctx.shell.println_plain(fmt_output(&out), Verbosity::Quiet);

                    if !out.status.success() {
                        errs += 1;
                    }
                }
                Err(e) => bail!("not all tests executed:\n{}", e),
            }
        }

        if errs != 0 {
            Err(format_err!(
                "{} test binaries executed with {} failures",
                root.len(),
                errs
            ))
        } else {
            Ok(format!("{} test binaries executed", root.len()))
        }
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
    backend: &Backend,
    force: bool,
) -> Res<String> {
    let f = |cache: &Cache, mut retriever: Retriever, solve| -> Res<String> {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let mut contents = String::new();
        let mut manifest = fs::File::open(sources[NodeIndex::new(0)].path().join("elba.toml"))
            .context(format_err!("failed to read manifest file (elba.toml)"))?;
        manifest.read_to_string(&mut contents)?;
        let manifest = Manifest::from_str(&contents)?;

        // By default, we build all bin targets.
        let mut root = vec![];
        let emp = targets.is_empty();
        for (ix, bt) in manifest.targets.bin.iter().enumerate() {
            if emp || targets.contains(&bt.name.as_str()) {
                root.push(Target::Bin(ix));
            }
        }
        let root = Targets::new(root);

        let bctx = BuildContext {
            backend,
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[2/3]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        // We unconditionally use a global OutputLayout to force rebuilding of root packages
        // and to avoid dealing with making our own for global/remote packages

        let q = JobQueue::new(sources, &root, None, &bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        let bins = q.exec(&bctx)?.1;
        let binc = bins.len();

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[3/3]").dim().bold(),
            "Installing binaries...",
            Verbosity::Quiet,
        );
        cache.store_bins(&bins, force)?;

        Ok(format!(
            "{} binaries installed into {}",
            binc,
            cache.layout.bin.display()
        ))
    };

    match name {
        Ok(name) => solve_remote(ctx, name, 3, f),
        Err(path) => solve_local(ctx, &path, 3, f),
    }
}

pub fn repl(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, Option<Vec<&str>>),
    backend: &Backend,
    ide: bool,
) -> Res<String> {
    let mut contents = String::new();
    let project = find_manifest_root(project)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    let mut imports = vec![];
    let mut paths = vec![];

    if let Some(lib) = manifest.targets.lib {
        if targets.1.is_none() || targets.0 {
            imports.push(lib.path.0.clone());
            paths.extend(lib.mods.iter().map(|mod_name| {
                let np: PathBuf = mod_name.replace(".", "/").into();
                lib.path.0.join(np).with_extension("idr")
            }));
        }
    }

    for bin in manifest.targets.bin {
        if let Some(v) = targets.1.as_ref() {
            if v.contains(&bin.name.as_ref()) {
                let resolved = bin.resolve_bin(Path::new(".")).ok_or_else(|| {
                    format_err!(
                        "module {} isn't a subpath and doesn't exist under path {}",
                        bin.main,
                        bin.path.0.display()
                    )
                })?;
                imports.push(resolved.0);
                paths.push(resolved.1);
            }
        } else if !targets.0 {
            let resolved = bin.resolve_bin(Path::new(".")).ok_or_else(|| {
                format_err!(
                    "module {} isn't a subpath and doesn't exist under path {}",
                    bin.main,
                    bin.path.0.display()
                )
            })?;
            imports.push(resolved.0);
            paths.push(resolved.1);
        }
    }

    solve_local(ctx, &project, 3, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        // We add no targets because we're going to directly add the paths of the files (so we can
        // interactively edit).
        let root = vec![];
        let root = Targets::new(root);

        let bctx = BuildContext {
            backend,
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[2/3]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        let mut q = JobQueue::new(sources, &root, None, &bctx, &ctx.logger, ctx.shell)?;

        // We only want to build the dependencies; we expressly do NOT want to generate anything
        // for the root package, because we're gonna manually add the files ourselves.
        // The reason we do this is because the repl is often used for interactive development.
        q.graph.inner[NodeIndex::new(0)] = Job::default();

        let deps = q.exec(&bctx)?.0;

        // From here, we basically manually build a CompileInvocation, but tailor-made for the
        // repl command.
        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[3/3]").dim().bold(),
            "Launching REPL...",
            Verbosity::Quiet,
        );
        ctx.shell.println_empty(Verbosity::Quiet);

        let mut process = bctx.compiler.process();
        for binary in deps {
            // We assume that the binary has already been compiled
            process.arg("-i").arg(binary);
        }
        for path in &imports {
            process.arg("-i").arg(path);
        }

        // We add the arguments passed by the environment variable IDRIS_OPTS at the end so that any
        // conflicting flags will be ignored (idris chooses the earliest flags first)
        if let Ok(val) = env::var("IDRIS_OPTS") {
            process.args(val.split(' ').collect::<Vec<_>>());
        }
        // Add the files we want to make available for the repl
        for target in &paths {
            process.arg(target);
        }

        if ide {
            process.arg("--ide-mode");
        }

        // The moment of truth:
        process
            .spawn()
            .with_context(|e| format_err!("couldn't launch the repl:\n{}", e))?
            .wait_with_output()
            .with_context(|e| format_err!("misc. repl failure:\n{}", e))?;

        // Clean up after ourselves
        for target in &paths {
            let bin = target.with_extension("ibc");
            if bin.exists() {
                fs::remove_file(&bin).with_context(|e| {
                    format_err!("couldn't remove ibc file {}:\n{}", bin.display(), e)
                })?;
            }
        }

        Ok("finished repl session".to_string())
    })
}

pub fn doc(ctx: &BuildCtx, project: &Path) -> Res<String> {
    let mut contents = String::new();
    let project = find_manifest_root(project)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    // By default, we build all lib and bin targets.
    let mut root = vec![];
    if manifest.targets.lib.is_some() {
        root.push(Target::Lib(false));
        root.push(Target::Doc);
    } else {
        // The user specifically asked for a lib target but there wasn't any. Error.
        bail!("the package doesn't have a library target. add one before proceeding")
    }
    let root = Targets::new(root);

    solve_local(ctx, &project, 2, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let backend = Backend::default();

        let bctx = BuildContext {
            // We just use the default backend cause it doesn't matter for this case
            backend: &backend,
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[2/2]").dim().bold(),
            "Building targets + root docs...",
            Verbosity::Quiet,
        );

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, &root, Some(layout), &bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec(&bctx)?;

        Ok("docs output available at `./target/docs`".to_string())
    })
}

pub fn build(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, bool, Option<Vec<&str>>, Option<Vec<&str>>),
    backend: &Backend,
) -> Res<String> {
    let mut contents = String::new();
    let project = find_manifest_root(project)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    // By default, we build all lib and bin targets.
    let mut root = vec![];
    if (targets.2.is_none() || targets.0 || targets.1) && manifest.targets.lib.is_some() {
        root.push(Target::Lib(targets.1));
    } else if targets.0 || targets.1 {
        // The user specifically asked for a lib target but there wasn't any. Error.
        bail!("the package doesn't have a library target. add one before proceeding")
    }

    if targets.2.as_ref().is_some() && manifest.targets.bin.is_empty() {
        // The user specifically asked for a bin target(s) but there wasn't any. Error.
        bail!("the package doesn't have any binary targets. add one before proceeding")
    }

    for (ix, bt) in manifest.targets.bin.iter().enumerate() {
        // Case 1: If the --bin flag is passed by itself, we assume the user wants all binaries.
        //         Or, the --bin flag might come with the name of a binary which we should build.
        let target_specified = targets
            .2
            .as_ref()
            .map(|v| v.is_empty() || v.contains(&bt.name.as_str()))
            .unwrap_or(false);
        // Case 2: Neither --bin nor --lib are specified. We're fine with --lib-cg.
        let neither_specified = !targets.0 && targets.2.is_none();
        if target_specified || neither_specified {
            root.push(Target::Bin(ix));
        }
    }

    // We only build test targets if the user asks for them.
    if let Some(ts) = &targets.3 {
        for (ix, bt) in manifest.targets.test.iter().enumerate() {
            let target_specified = ts.is_empty() || ts.contains(&bt.name.as_str());
            if target_specified {
                root.push(Target::Test(ix));
            }
        }
    }

    let root = Targets::new(root);
    solve_local(ctx, &project, 2, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let bctx = BuildContext {
            backend,
            compiler: Compiler::default(),
            config: BuildConfig {},
            cache,
            threads: ctx.threads,
        };

        ctx.shell.println_empty(Verbosity::Normal);
        ctx.shell.println(
            style("[2/2]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, &root, Some(layout), &bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec(&bctx)?;

        Ok("build output available at `./target`".to_string())
    })
}

pub fn lock(ctx: &BuildCtx, project: &Path) -> Res<String> {
    solve_local(ctx, &project, 1, |_, _, _| {
        Ok("lockfile created at `./elba.lock`".to_string())
    })
}

pub fn solve_local<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Res<String>>(
    ctx: &BuildCtx,
    project: &Path,
    total: u8,
    mut f: F,
) -> Res<String> {
    let project = find_manifest_root(project)?;
    let mut manifest = fs::File::open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    let mut contents = String::new();
    manifest.read_to_string(&mut contents)?;

    let manifest = Manifest::from_str(&contents)?;

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

    let root = {
        let cur = project.clone();
        let pid = PackageId::new(manifest.name().clone(), DirectRes::Dir { path: cur }.into());
        Summary::new(pid, manifest.version().clone())
    };

    let def_index = def_index(ctx);

    let deps = manifest
        .deps(&def_index, true)
        .into_iter()
        .collect::<Vec<_>>();

    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), ctx.shell)?;

    ctx.shell.println(
        style(format!("[1/{}]", total)).dim().bold(),
        "Resolving dependencies...",
        Verbosity::Quiet,
    );

    let indices = cache.get_indices(&ctx.indices, ctx.offline);
    ctx.shell.println(
        style("Cached").dim(),
        format!("indices at {}", cache.layout.indices.display()),
        Verbosity::Verbose,
    );

    let mut retriever = Retriever::new(
        &cache.logger,
        &cache,
        root,
        deps,
        indices,
        lock,
        def_index,
        ctx.shell,
        ctx.offline,
    );
    let solver = Resolver::new(&retriever.logger.clone(), &mut retriever);
    let solve = solver.solve()?;
    ctx.shell.println(
        style("Writing").dim(),
        "lockfile at elba.lock",
        Verbosity::Verbose,
    );

    let mut lockfile = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(project.join("elba.lock"))
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
    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), ctx.shell)?;
    ctx.shell.println(
        style(format!("[1/{}]", total)).dim().bold(),
        "Resolving dependencies...",
        Verbosity::Quiet,
    );
    let mut indices = cache.get_indices(&ctx.indices, ctx.offline);
    ctx.shell.println(
        style("Cached").dim(),
        format!("indices at {}", cache.layout.indices.display()),
        Verbosity::Verbose,
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

    let mut retriever = Retriever::new(
        &cache.logger,
        &cache,
        root,
        deps,
        indices,
        lock,
        def_index,
        ctx.shell,
        ctx.offline,
    );
    let solve = Resolver::new(&retriever.logger.clone(), &mut retriever).solve()?;

    ctx.shell.println_empty(Verbosity::Normal);

    f(&cache, retriever, solve)
}

fn def_index(ctx: &BuildCtx) -> IndexRes {
    if ctx.indices.is_empty() {
        IndexRes::from_str("index+dir+none").unwrap()
    } else {
        ctx.indices[0].clone().into()
    }
}

fn find_manifest_root(path: &Path) -> Res<PathBuf> {
    for p in path.ancestors() {
        if p.join("elba.toml").exists() {
            return Ok(p.to_path_buf());
        }
    }

    Err(format_err!(
        "no manifest file (elba.toml) exists in any parent directory"
    ))
}
