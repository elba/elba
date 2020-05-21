use std::{
    convert::TryInto,
    env, fs,
    io::{prelude::*, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use console::style;
use crossbeam::queue::MsQueue;
use failure::{bail, format_err, ResultExt};
use indexmap::IndexMap;
use itertools::Either::{self, Left, Right};
use petgraph::{graph::NodeIndex, visit::Dfs};
use scoped_threadpool::Pool;
use slog::Logger;
use toml;
use toml_edit;

use crate::{
    build::{
        context::{BuildContext, Compiler},
        job::{Job, JobQueue},
        Target, Targets,
    },
    package::{
        ipkg::Ipkg,
        lockfile::LockfileToml,
        manifest::{BinTarget, Manifest},
        PackageId, Spec, Summary,
    },
    remote::resolution::{DirectRes, IndexRes, Resolution},
    resolve::Resolver,
    retrieve::{
        cache::{Cache, Layout, OutputLayout},
        Retriever,
    },
    util::{
        config::Backend,
        error::Result,
        fmt_output,
        graph::Graph,
        lock::DirLock,
        shell::{Shell, Verbosity},
    },
};

pub struct BuildCtx {
    pub compiler: String,
    pub indices: IndexMap<String, IndexRes>,
    pub global_cache: Layout,
    pub logger: Logger,
    pub threads: u32,
    pub shell: Shell,
    pub offline: bool,
    pub opts: Vec<String>,
}

pub fn test(
    ctx: &BuildCtx,
    project: &Path,
    targets: &[&str],
    backend: &Backend,
    test_threads: u32,
) -> Result<String> {
    let (project, manifest) = find_manifest(project, true, None)?;

    if manifest.targets.test.is_empty() {
        bail!("at least one test must be defined")
    }

    solve_local(&ctx, &project, 3, None, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let bctx = BuildContext {
            backend: backend.clone(),
            codegen: true,
            compiler: Compiler::new(&ctx.compiler)?,
            opts: ctx.opts.clone(),
            cache: cache.clone(),
            threads: ctx.threads,
        };

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
            let bt: BinTarget = bt.clone().into();
            if emp || targets.contains(&bt.name.as_str()) {
                root.push(Target::Test(ix));
            }
        }

        let root = Targets::new(root);
        let q = JobQueue::new(sources, &root, Some(layout), bctx, &ctx.logger, ctx.shell)?;
        q.exec()?;

        ctx.shell.println(
            style("[3/3]").dim().bold(),
            "Running tests...",
            Verbosity::Quiet,
        );

        let root: Vec<BinTarget> = root
            .0
            .into_iter()
            .filter_map(|t| {
                if let Target::Test(ix) = t {
                    Some(manifest.targets.test[ix].clone().into())
                } else {
                    None
                }
            })
            .collect();

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
                let runner = &backend.runner;
                // let pb = &pb;
                scope.execute(move || {
                    shell.println(style("Running").cyan(), &test.name, Verbosity::Normal);
                    let out = if let Some(r) = runner {
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

pub fn install(
    ctx: &BuildCtx,
    name: Either<Spec, PathBuf>,
    targets: &[&str],
    backend: &Backend,
    force: bool,
) -> Result<String> {
    let f = |cache: &Cache, mut retriever: Retriever, solve| -> Result<String> {
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
            backend: backend.clone(),
            codegen: true,
            compiler: Compiler::new(&ctx.compiler)?,
            opts: ctx.opts.clone(),
            cache: cache.clone(),
            threads: ctx.threads,
        };

        ctx.shell.println(
            style("[2/3]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        // We unconditionally use a global OutputLayout to force rebuilding of root packages
        // and to avoid dealing with making our own for global/remote packages

        let q = JobQueue::new(sources, &root, None, bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        let bins = q.exec()?.1;
        let binc = bins.len();

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
        Left(name) => solve_remote(ctx, &name, 3, f),
        Right(path) => solve_local(ctx, &path, 3, None, f),
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Interactivity {
    Normal,
    IDE,
    Socket,
}

pub fn repl(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, Option<Vec<&str>>),
    backend: &Backend,
    interactivity: Interactivity,
) -> Result<String> {
    let (project, manifest) = find_manifest(project, true, None)?;

    env::set_current_dir(&project)?;

    let mut parents = vec![];
    let mut paths = vec![];

    if let Some(lib) = manifest.targets.lib {
        if targets.1.is_none() || targets.0 {
            let src_path = lib.path.0.clone();
            let new_paths = lib
                .mods
                .iter()
                .map(|mod_name| {
                    let path: PathBuf = mod_name.trim_matches('.').replace(".", "/").into();
                    if src_path.join(&path).with_extension("idr").exists() {
                        Ok((parents.len(), path.with_extension("idr")))
                    } else if src_path.join(&path).with_extension("lidr").exists() {
                        Ok((parents.len(), path.with_extension("lidr")))
                    } else {
                        Err(format_err!(
                            "Module at path {} doesn't exist",
                            path.display()
                        ))
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            parents.push(src_path);
            paths.extend(new_paths);
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
                parents.push(resolved.0);
                paths.push((parents.len() - 1, resolved.1));
            }
        } else if !targets.0 {
            let resolved = bin.resolve_bin(Path::new(".")).ok_or_else(|| {
                format_err!(
                    "module {} isn't a subpath and doesn't exist under path {}",
                    bin.main,
                    bin.path.0.display()
                )
            })?;
            parents.push(resolved.0);
            paths.push((parents.len() - 1, resolved.1));
        }
    }

    solve_local(ctx, &project, 3, None, |cache, mut retriever, solve| {
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
            backend: backend.clone(),
            codegen: true,
            compiler: Compiler::new(&ctx.compiler)?,
            opts: ctx.opts.clone(),
            cache: cache.clone(),
            threads: ctx.threads,
        };

        ctx.shell.println(
            style("[2/3]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        let mut q = JobQueue::new(sources, &root, None, bctx.clone(), &ctx.logger, ctx.shell)?;

        // We only want to build the dependencies; we expressly do NOT want to generate anything
        // for the root package, because we're gonna manually add the files ourselves.
        // The reason we do this is because the repl is often used for interactive development.
        q.graph.inner[NodeIndex::new(0)] = Job::default();

        let deps = q.exec()?.0;

        // From here, we basically manually build a CompileInvocation, but tailor-made for the
        // repl command.
        ctx.shell.println(
            style("[3/3]").dim().bold(),
            "Launching REPL...",
            Verbosity::Quiet,
        );

        if bctx.compiler.flavor().is_idris2() {
            bail!("The Idris 2 compiler doesn't currently support custom source paths, needed for the REPL.")
        }

        let mut process = bctx.compiler.process();
        for binary in deps {
            // We assume that deps have already been compiled
            process.arg("-i").arg(binary);
        }

        for path in &parents {
            process.arg("--sourcepath").arg(path);
            process.arg("-i").arg(path);
        }

        // We add the arguments in the build context at the end so that any
        // conflicting flags will be ignored (idris chooses the earliest flags first)
        process.args(&ctx.opts);

        // Add the files we want to make available for the repl
        for target in &paths {
            process.arg(&target.1);
        }

        match interactivity {
            // In ide-mode, we only want to pass the current file as the arg.
            // An editor should be in charge of dealing with this.
            Interactivity::IDE => {
                process.arg("--ide-mode");
            }
            Interactivity::Socket => {
                process.arg("--ide-mode-socket");
            }
            _ => {}
        };

        // The moment of truth:
        process
            .spawn()
            .with_context(|e| format_err!("couldn't launch the repl:\n{}", e))?
            .wait_with_output()
            .with_context(|e| format_err!("misc. repl failure:\n{}", e))?;

        // Clean up after ourselves
        for target in &paths {
            let src_path = &parents[target.0];
            let bin = src_path.join(&target.1).with_extension("ibc");
            if bin.exists() {
                fs::remove_file(&bin).with_context(|e| {
                    format_err!("couldn't remove ibc file {}:\n{}", bin.display(), e)
                })?;
            }
        }

        Ok("finished repl session".to_string())
    })
}

pub fn doc(ctx: &BuildCtx, project: &Path) -> Result<String> {
    let (project, manifest) = find_manifest(project, true, None)?;

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

    solve_local(ctx, &project, 2, None, |cache, mut retriever, solve| {
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
            backend,
            codegen: true,
            compiler: Compiler::new(&ctx.compiler)?,
            opts: ctx.opts.clone(),
            cache: cache.clone(),
            threads: ctx.threads,
        };

        ctx.shell.println(
            style("[2/2]").dim().bold(),
            "Building targets + root docs...",
            Verbosity::Quiet,
        );

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, &root, Some(layout), bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec()?;

        Ok("docs output available at `./target/docs`".to_string())
    })
}

pub fn build(
    ctx: &BuildCtx,
    project: &Path,
    targets: &(bool, bool, Option<Vec<&str>>, Option<Vec<&str>>),
    codegen: bool,
    backend: &Backend,
) -> Result<String> {
    let (project, manifest) = find_manifest(project, true, None)?;

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
            let bt: BinTarget = bt.clone().into();
            let target_specified = ts.is_empty() || ts.contains(&bt.name.as_str());
            if target_specified {
                root.push(Target::Test(ix));
            }
        }
    }

    let root = Targets::new(root);
    solve_local(ctx, &project, 2, None, |cache, mut retriever, solve| {
        let sources = retriever
            .retrieve_packages(&solve)
            .context(format_err!("package retrieval failed"))?;

        // We drop the Retriever because we want to release our lock on the Indices as soon as we
        // can to avoid stopping other instances of elba from downloading and resolving (even
        // though we don't even need the Retriever anymore).
        drop(retriever);

        let bctx = BuildContext {
            backend: backend.clone(),
            codegen,
            compiler: Compiler::new(&ctx.compiler)?,
            opts: ctx.opts.clone(),
            cache: cache.clone(),
            threads: ctx.threads,
        };

        ctx.shell.println(
            style("[2/2]").dim().bold(),
            "Building targets...",
            Verbosity::Quiet,
        );

        // We want to store the outputs of our labor in a local target directory.
        let lock = DirLock::acquire(&project.join("target"))?;
        let layout = OutputLayout::new(lock).context("could not create local target directory")?;

        let q = JobQueue::new(sources, &root, Some(layout), bctx, &ctx.logger, ctx.shell)?;
        // Because we're just building, we don't need to do anything after executing the build
        // process. Yay abstraction!
        q.exec()?;

        Ok("build output available at `./target`".to_string())
    })
}

pub fn update(ctx: &BuildCtx, project: &Path, ignore: Option<&[Spec]>) -> Result<String> {
    let (project, _) = find_manifest(project, true, None)?;

    let op = || -> Result<Graph<Summary>> {
        let mut f = fs::File::open(&project.join("elba.lock"))?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        let toml = LockfileToml::from_str(&contents)?;

        Ok(toml.into())
    };

    let prev = op().ok();

    solve_local(ctx, &project, 1, ignore, |_, _, solve| {
        if let Some(prev) = prev.as_ref() {
            for (_, old) in prev.sub_tree(NodeIndex::new(0)) {
                if let Some(new) = solve.find_by(|sum| sum.id().lowkey_eq(old.id())) {
                    if old.id() != new.id() {
                        // This is a git repo or something
                        ctx.shell.println(
                            style("Updating").cyan(),
                            format!("{} -> {}", old, new),
                            Verbosity::Normal,
                        );
                    } else if old != new {
                        ctx.shell.println(
                            style("Updating").cyan(),
                            format!(
                                "{} ({}) {} -> {}",
                                old.name(),
                                old.resolution(),
                                old.version(),
                                new.version()
                            ),
                            Verbosity::Normal,
                        );
                    }
                // Otherwise, the packages are exactly the same in both, and we don't do
                // anything
                } else {
                    // It's in the old, but not the new. It was removed!
                    ctx.shell
                        .println(style("Removing").red(), old, Verbosity::Normal);
                }
            }

            for (_, new) in prev.sub_tree(NodeIndex::new(0)) {
                // At this point we just want to find packages which were added in the new lockfile
                if solve.find_by(|sum| new.id().lowkey_eq(sum.id())).is_none() {
                    ctx.shell
                        .println(style("Adding").green(), new, Verbosity::Normal);
                }
            }

            Ok("lockfile at ./elba.lock updated".to_string())
        } else {
            Ok("lockfile created at `./elba.lock`".to_string())
        }
    })
}

pub fn add(ctx: &BuildCtx, project: &Path, spec: &Spec, dev: bool) -> Result<String> {
    let mut contents = String::new();
    let (project, _) = find_manifest(project, true, None)?;
    let mut mf = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(project.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    mf.read_to_string(&mut contents)?;
    let mut manifest = contents
        .parse::<toml_edit::Document>()
        .with_context(|e| format!("invalid manifest toml format: {}", e))?;

    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), ctx.shell)?;
    let indices = ctx
        .indices
        .values()
        .cloned()
        .map(|x| x.res)
        .collect::<Vec<_>>();
    let indices = cache.get_indices(&indices, false, false);
    let target = indices.select_by_spec(&spec)?;
    let target_s = target.to_string();

    let res = match target.id.resolution() {
        Resolution::Index(IndexRes { res }) => res.clone(),
        _ => unreachable!(),
    };

    if !dev {
        manifest["dependencies"][target.id.name().to_string()]["version"] =
            toml_edit::value(target.version.to_string());
        manifest["dependencies"][target.id.name().to_string()]["index"] =
            toml_edit::value(res.to_string());
        manifest["dependencies"][target.id.name().to_string()]
            .as_inline_table_mut()
            .map(|t| t.fmt());
    } else {
        manifest["dev_dependencies"][target.id.name().to_string()]["version"] =
            toml_edit::value(target.version.to_string());
        manifest["dev_dependencies"][target.id.name().to_string()]["index"] =
            toml_edit::value(res.to_string());
        manifest["dev_dependencies"][target.id.name().to_string()]
            .as_inline_table_mut()
            .map(|t| t.fmt());
    }

    mf.seek(SeekFrom::Start(0))?;
    mf.write_all(manifest.to_string().as_bytes())?;

    Ok(format!("added package {} to manifest", target_s))
}

pub fn solve_local<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Result<String>>(
    ctx: &BuildCtx,
    project: &Path,
    total: u8,
    ignore: Option<&[Spec]>,
    mut f: F,
) -> Result<String> {
    let (project, manifest) = find_manifest(project, true, Some(ctx.shell))?;

    let op = || -> Result<Graph<Summary>> {
        let mut f = fs::File::open(&project.join("elba.lock"))?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        let toml = LockfileToml::from_str(&contents)?;

        Ok(toml.into())
    };

    let lock = match ignore {
        None => {
            if let Ok(solve) = op() {
                solve
            } else {
                Graph::default()
            }
        }
        Some(i) => {
            if i.is_empty() {
                Graph::default()
            } else if let Ok(mut solve) = op() {
                for spec in i {
                    let mut chosen: Option<Summary> = None;
                    let mut dfs = Dfs::new(&solve.inner, NodeIndex::new(0));
                    while let Some(ix) = dfs.next(&solve.inner) {
                        if spec.matches(&solve[ix]) {
                            if let Some(already_chosen) = chosen {
                                return Err(format_err!(
                                    "spec {} is ambiguous: both {} and {} match",
                                    spec,
                                    &solve[ix],
                                    already_chosen
                                ));
                            } else {
                                chosen = Some(solve.inner.remove_node(ix).unwrap());
                            }
                        }
                    }
                    if chosen.is_none() {
                        return Err(format_err!("spec {} not in lockfile", spec));
                    }
                }
                solve
            } else {
                Graph::default()
            }
        }
    };

    let root = {
        let cur = project.clone();
        let pid = PackageId::new(manifest.name().clone(), DirectRes::Dir { path: cur }.into());
        Summary::new(pid, manifest.version().clone())
    };

    let deps = manifest
        .deps(&ctx.indices, &root.id, true)?
        .into_iter()
        .collect::<Vec<_>>();

    let dreses = deps
        .iter()
        .filter_map(|(p, _)| {
            if let Resolution::Index(IndexRes { res }) = p.resolution() {
                Some(res.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), ctx.shell)?;

    ctx.shell.println(
        style(format!("[1/{}]", total)).dim().bold(),
        "Resolving dependencies...",
        Verbosity::Quiet,
    );

    let mut retriever = Retriever::new(
        &cache.logger,
        &cache,
        root,
        deps,
        Left(dreses),
        lock,
        &ctx.indices,
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

pub fn solve_remote<F: FnMut(&Cache, Retriever, Graph<Summary>) -> Result<String>>(
    ctx: &BuildCtx,
    name: &Spec,
    total: u8,
    mut f: F,
) -> Result<String> {
    let cache = Cache::from_disk(&ctx.logger, ctx.global_cache.clone(), ctx.shell)?;
    ctx.shell.println(
        style(format!("[1/{}]", total)).dim().bold(),
        "Resolving dependencies...",
        Verbosity::Quiet,
    );
    // For remote packages, we check the config for the indices we can load from
    let indices = ctx
        .indices
        .values()
        .cloned()
        .map(|x| x.res)
        .collect::<Vec<_>>();
    let mut indices = cache.get_indices(&indices, true, ctx.offline);
    ctx.shell.println(
        style("Cached").dim(),
        format!("indices at {}", cache.layout.indices.display()),
        Verbosity::Verbose,
    );
    let root = indices.select_by_spec(&name)?;

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
        Right(indices),
        lock,
        &ctx.indices,
        ctx.shell,
        ctx.offline,
    );
    let solve = Resolver::new(&retriever.logger.clone(), &mut retriever).solve()?;

    f(&cache, retriever, solve)
}

pub fn find_manifest(
    path: &Path,
    allow_ipkg: bool,
    shell: Option<Shell>,
) -> Result<(PathBuf, Manifest)> {
    let root = path.ancestors().find(|p| p.join("elba.toml").exists());
    match root {
        Some(root) => {
            let toml_path = root.join("elba.toml");
            let mut file = fs::File::open(&toml_path).context(format_err!(
                "failed to read manifest file ({})",
                toml_path.display()
            ))?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let manifest = Manifest::from_str(&contents)?;
            Ok((root.to_path_buf(), manifest))
        }
        None if allow_ipkg => {
            let ipkgs: Vec<PathBuf> = fs::read_dir(path)?
                .into_iter()
                .filter_map(|entry| Some(entry.ok()?.path().to_path_buf()))
                .filter(|p| p.extension().map(|p| p.to_str()).flatten() == Some("ipkg"))
                .collect();
            if ipkgs.len() == 0 {
                return Err(format_err!(
                    "no manifest file (elba.toml) exists in any parent directory and \
                    no ipkg file exists in current directory"
                ));
            }
            if ipkgs.len() > 1 {
                return Err(format_err!(
                    "no manifest file (elba.toml) exists in any parent directory and \
                    multiple ipkg files are found (only one is allowed)"
                ));
            }
            let mut file = fs::File::open(path.join(ipkgs.get(0).unwrap()))
                .context(format_err!("failed to read ipkg file"))?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let ipkg = Ipkg::from_str(&contents).context(format_err!("while parsing ipkg file"))?;
            let manifest: Manifest = ipkg.try_into()?;

            if let Some(shell) = shell {
                shell.println(
                    style("[warn]").yellow().bold(),
                    format!(
                        "Loaded a legacy package {} {} from ipkg file",
                        &manifest.package.name, &manifest.package.version
                    ),
                    Verbosity::Normal,
                );
            }

            Ok((path.to_path_buf(), manifest))
        }
        None => Err(format_err!(
            "no manifest file (elba.toml) exists in any parent directory"
        )),
    }
}
