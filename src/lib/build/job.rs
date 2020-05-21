use super::{compile_bin, compile_doc, compile_lib, context::BuildContext, Target, Targets};
use crate::{
    retrieve::cache::{Binary, BuildHash, OutputLayout, Source},
    util::{
        clear_dir,
        error::Result,
        fmt_multiple,
        graph::Graph,
        lock::DirLock,
        shell::{Shell, Verbosity},
    },
};
use console::style;
use failure::{bail, format_err, ResultExt};
use futures::future;
use petgraph::graph::NodeIndex;
use slog::{debug, o, Logger};
use std::{collections::HashSet, future::Future, path::PathBuf};
use tokio::runtime::Runtime;

/// Work refers to either a Source and its BuildHash which needs to be built,
/// a built library which is still being used by other code, or a built target
/// with no remaining dependencies up the chain.
///
/// We separate things like this to drop our locks on directories as soon as we
/// can, to allow other instances of elba to start work asap.
#[derive(Debug, PartialEq, Eq)]
pub enum Work {
    None,
    Fresh(Binary),
    Dirty(Source, BuildHash),
}

impl Work {
    pub fn is_none(&self) -> bool {
        match self {
            Work::None => true,
            _ => false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            Work::Dirty(_, _) => true,
            _ => false,
        }
    }

    pub fn is_fresh(&self) -> bool {
        match self {
            Work::Fresh(_) => true,
            _ => false,
        }
    }
}

/// A Job is an individual unit of work in the elba build graph.
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub work: Work,
    pub targets: Targets,
}

impl Default for Job {
    fn default() -> Self {
        Job {
            work: Work::None,
            targets: Targets::new(vec![Target::Lib(false)]),
        }
    }
}

pub struct JobQueue {
    /// The graph of jobs which need to be done.
    pub graph: Graph<Job>,
    pub root_ol: Option<OutputLayout>,
    pub logger: Logger,
    pub shell: Shell,
    pub bcx: BuildContext,
}

// The current implementation of the JobQueue combines target generation and dependency preparation
// into one big Job graph.
impl JobQueue {
    pub fn new(
        solve: Graph<Source>,
        root: &Targets,
        root_ol: Option<OutputLayout>,
        bcx: BuildContext,
        plog: &Logger,
        shell: Shell,
    ) -> Result<Self> {
        let mut graph = Graph::new(solve.inner.map(|_, _| Job::default(), |_, _| ()));

        let mut curr_layer = HashSet::new();
        let mut next_layer = HashSet::new();

        // We start with the root node.
        next_layer.insert(NodeIndex::new(0));

        let ver = bcx.compiler.version();

        if let Err(e) = &ver {
            shell.println(style("[warn]").yellow().bold(), e, Verbosity::Normal);
        }

        let ver = ver.ok();
        let logger = plog.new(o!(
            "phase" => "build",
            "threads" => bcx.threads,
            "compiler" => ver.clone().unwrap_or_else(|| "none".to_string())
        ));

        while !next_layer.is_empty() {
            debug_assert!(curr_layer.is_empty());

            curr_layer.extend(next_layer.drain());

            for node in curr_layer.drain() {
                let source = &solve[node];

                let targets = if node == NodeIndex::new(0) {
                    root.clone()
                } else {
                    Targets::new(vec![Target::Lib(false)])
                };

                let build_hash = BuildHash::new(
                    source,
                    &solve,
                    &targets,
                    &bcx,
                    (node != NodeIndex::new(0) || bcx.codegen) && targets.is_codegen(),
                );

                let root_ol = root_ol.as_ref();
                let job = if node == NodeIndex::new(0)
                    && root_ol.is_some()
                    && root_ol.unwrap().is_built(&build_hash)
                {
                    Job {
                        work: Work::None,
                        targets,
                    }
                } else {
                    match bcx.cache.checkout_build(&build_hash)? {
                        Some(binary) => Job {
                            work: Work::Fresh(binary),
                            targets,
                        },
                        None => {
                            next_layer.extend(
                                graph
                                    .children(node)
                                    // If the Job is none, that means that it hasn't been visited yet.
                                    .filter(|(_, child)| child.work.is_none())
                                    .map(|(index, _)| index),
                            );

                            Job {
                                work: Work::Dirty(source.clone(), build_hash),
                                targets,
                            }
                        }
                    }
                };
                graph[node] = job;
            }
        }

        // We drop the all of the Sources, releasing our lock on them. We don't need them anymore.
        drop(solve);

        Ok(JobQueue {
            graph,
            root_ol,
            bcx,
            logger,
            shell,
        })
    }

    pub fn exec<'a>(self) -> Result<(Vec<PathBuf>, Vec<(PathBuf, String)>)> {
        let mut rt =
            Runtime::new().with_context(|_| format_err!("Couldn't start parallel runtime"))?;
        rt.block_on(self.exec_async())
    }

    async fn exec_async<'a>(mut self) -> Result<(Vec<PathBuf>, Vec<(PathBuf, String)>)> {
        let root_ol = &self.root_ol;
        let root_hash = self.graph.root().and_then(|x| {
            if let Work::Dirty(_, h) = &x.work {
                Some(h.clone())
            } else {
                None
            }
        });

        let mut ongoing_jobs: HashSet<NodeIndex> = HashSet::new();
        let mut parallal_jobs_future = Vec::new();
        let mut bins_vec = Vec::new();

        loop {
            // Bottom jobs are Dirty jobs whose dependencies are all satisfied.
            let bottom_jobs = self.graph.inner.node_indices().filter(|&index| {
                self.graph[index].work.is_dirty()
                    && self
                        .graph
                        .children(index)
                        .all(|(child, _)| self.graph[child].work.is_fresh())
            });

            // Spwan new jobs
            for job in bottom_jobs {
                if !ongoing_jobs.contains(&job) {
                    parallal_jobs_future.push(Box::pin(self.complete_job(job)?));
                    ongoing_jobs.insert(job);
                }
            }

            // Check if build is complete
            if ongoing_jobs.is_empty() {
                break;
            }

            // Await one of the jobs to complete
            let (job_res, _, remaining) = future::select_all(parallal_jobs_future).await;
            parallal_jobs_future = remaining;

            // Handle the job result
            match job_res {
                Ok((job_index, binary, mut bins)) => {
                    ongoing_jobs.remove(&job_index);

                    // prg += 1;
                    // pb.set_position(prg);
                    if let Some(b) = binary {
                        // If we got a compiled library out of it, set the binary
                        self.graph[job_index].work = Work::Fresh(b)
                    } else if self.graph[job_index].work.is_dirty() {
                        // The Targets struct ensures that the library target will always be
                        // compiled first, meaning that the work will be set to Fresh.
                        // If we're compiling a not-library and it's still dirty, that means
                        // there's no lib target and nothing to depend on.
                        self.graph[job_index].work = Work::None
                    }

                    // For all of this package's dependencies, if all of the packages which
                    // depend on them have been built, we can drop them entirely.
                    let mut child_done = Vec::new();
                    for (child, _) in self.graph.children(job_index) {
                        let ready = self
                            .graph
                            .parents(child)
                            .all(|(_, job)| job.work.is_fresh());

                        if ready {
                            child_done.push(child);
                        }
                    }
                    for child in child_done {
                        self.graph[child].work = Work::None;
                    }

                    bins_vec.append(&mut bins);
                }
                Err(err) => {
                    // pb.finish_and_clear();
                    self.shell
                        .println(style("[error]").red().bold(), err, Verbosity::Quiet);
                    bail!("one or more packages couldn't be built");
                }
            }
        }

        // Clean up the build environment
        if let Some(ol) = root_ol.as_ref() {
            let res = clear_dir(&ol.build);
            if let Err(e) = res {
                self.shell.println(
                    style("[warn]").yellow().bold(),
                    format!(
                        "Couldn't clear build directory {}: {}",
                        ol.build.display(),
                        e
                    ),
                    Verbosity::Normal,
                );
            }

            if let Some(r) = root_hash {
                let res = ol.write_hash(&r);
                if let Err(e) = res {
                    self.shell.println(
                        style("[warn]").yellow().bold(),
                        format!(
                            "Couldn't write build hash (root will be rebuilt on next run): {}",
                            e
                        ),
                        Verbosity::Normal,
                    );
                }
            }
        }

        let root_children = self
            .graph
            .children(NodeIndex::new(0))
            .filter_map(|(_, j)| {
                if let Work::Fresh(b) = &j.work {
                    Some(b.target.path().to_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Ok((root_children, bins_vec))
    }

    // Drive a job from dirty to done
    fn complete_job(
        &self,
        job_index: NodeIndex,
    ) -> Result<impl Future<Output = Result<(NodeIndex, Option<Binary>, Vec<(PathBuf, String)>)>>>
    {
        if let Work::Dirty(source, build_hash) = &self.graph[job_index].work {
            self.shell.println(
                style("Building").cyan(),
                format!("{} [{}..]", source.pretty_summary(), &build_hash.0[0..8]),
                Verbosity::Normal,
            );
            let layout: OutputLayout = if job_index == NodeIndex::new(0) {
                if let Some(x) = &self.root_ol {
                    x.clone()
                } else {
                    self.bcx.cache.checkout_tmp(&build_hash)?
                }
            } else {
                self.bcx.cache.checkout_tmp(&build_hash)?
            };

            let deps = self
                .graph
                .children(job_index)
                .filter(|(_, job)| job.work.is_fresh())
                .map(|(_, job)| match &job.work {
                    Work::Fresh(binary) => binary.clone(),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>();

            let targets = self.graph[job_index].targets.clone();

            let res = Self::compile_target(
                job_index,
                source.clone(),
                build_hash.clone(),
                targets,
                deps,
                layout,
                self.root_ol.is_some(),
                self.logger.clone(),
                self.bcx.clone(),
                self.shell,
            );

            Ok(res)
        } else {
            unreachable!()
        }
    }

    async fn compile_target<'a>(
        job_index: NodeIndex,
        source: Source,
        build_hash: BuildHash,
        targets: Targets,
        deps: Vec<Binary>,
        layout: OutputLayout,
        is_root: bool,
        logger: Logger,
        bcx: BuildContext,
        shell: Shell,
    ) -> Result<(NodeIndex, Option<Binary>, Vec<(PathBuf, String)>)> {
        let mut res: Option<Binary> = None;
        let mut bins: Vec<(PathBuf, String)> = Vec::new();
        let has_lib = targets.has_lib();

        for target in targets.0 {
            match target {
                Target::Lib(cg) => {
                    debug!(
                        logger, "building target";
                        "target_type" => "lib",
                        "target" => cg,
                        "summary" => source.summary()
                    );
                    let out = compile_lib(&source, cg, &deps, &layout, &bcx, shell)
                        .await
                        .with_context(|e| {
                            format!(
                                "Couldn't build library target for {}\n{}",
                                source.pretty_summary(),
                                e
                            )
                        })?;

                    res = if job_index == NodeIndex::new(0) && is_root {
                        let out = fmt_multiple(&out);
                        shell.println_plain(out, Verbosity::Normal);

                        let target = DirLock::acquire(&layout.lib)?;
                        Some(Binary::new(target))
                    } else {
                        Some(bcx.cache.store_build(&layout.lib, &build_hash)?)
                    }
                }
                Target::Bin(ix) => {
                    debug!(
                        logger, "building target";
                        "target_type" => "bin",
                        "target" => ix,
                        "summary" => source.summary()
                    );
                    let mut deps = deps.clone();
                    let root_lib;
                    if has_lib {
                        root_lib = {
                            let target = DirLock::acquire(&layout.lib)?;
                            Binary::new(target)
                        };
                        deps.push(root_lib);
                    }
                    let (out, path) =
                        compile_bin(&source, Target::Bin(ix), &deps, &layout, &bcx, shell)
                            .await
                            .with_context(|e| {
                                format!(
                                    "Couldn't build binary {} for {}\n{}",
                                    ix,
                                    source.pretty_summary(),
                                    e
                                )
                            })?;

                    if let Some(p) = path {
                        bins.push((p, source.summary()));
                    }

                    if job_index == NodeIndex::new(0) && is_root {
                        let out = fmt_multiple(&out);
                        shell.println_plain(out, Verbosity::Normal);
                    }
                }
                Target::Test(ix) => {
                    debug!(
                        logger, "building target";
                        "target_type" => "test",
                        "target" => ix,
                        "summary" => source.summary()
                    );
                    let mut deps = deps.clone();
                    let root_lib;
                    if has_lib {
                        root_lib = {
                            let target = DirLock::acquire(&layout.lib)?;
                            Binary::new(target)
                        };
                        deps.push(root_lib);
                    }
                    let (out, _) =
                        compile_bin(&source, Target::Test(ix), &deps, &layout, &bcx, shell)
                            .await
                            .with_context(|e| {
                                format!(
                                    "Couldn't build test {} for {}\n{}",
                                    ix,
                                    source.pretty_summary(),
                                    e
                                )
                            })?;

                    if job_index == NodeIndex::new(0) && is_root {
                        let out = fmt_multiple(&out);
                        shell.println_plain(out, Verbosity::Normal);
                    }

                    // For now, only the root package can do tests, so we
                    // don't worry about storing the binary anywhere.
                }
                Target::Doc => {
                    debug!(
                        logger, "building target";
                        "target_type" => "doc",
                        "summary" => source.summary()
                    );
                    let out = compile_doc(&source, &deps, &layout, &bcx)
                        .await
                        .with_context(|e| {
                            format!("Couldn't build docs for {}\n{}", source.pretty_summary(), e)
                        })?;

                    if job_index == NodeIndex::new(0) && is_root {
                        let out_str = fmt_multiple(&out);
                        shell.println_plain(out_str, Verbosity::Normal);
                    }
                }
            }
        }

        Ok((job_index, res, bins))
    }
}
