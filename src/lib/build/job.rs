use super::{compile_bin, compile_doc, compile_lib, context::BuildContext, Target, Targets};
use console::style;
use crossbeam::queue::MsQueue;
use failure::ResultExt;
use indicatif::{ProgressBar, ProgressStyle};
use petgraph::graph::NodeIndex;
use retrieve::cache::OutputLayout;
use retrieve::cache::{Binary, BuildHash, Source};
use scoped_threadpool::Pool;
use std::iter::FromIterator;
use std::{collections::HashSet, path::PathBuf};
use util::{clear_dir, errors::Res, fmt_output, graph::Graph, lock::DirLock};

pub struct JobQueue {
    /// The graph of jobs which need to be done.
    pub graph: Graph<Job>,
    pub root_ol: Option<OutputLayout>,
}

// The current implementation of the JobQueue combines target generation and dependency preparation
// into one big Job graph.
impl JobQueue {
    pub fn new(
        solve: Graph<Source>,
        root: &Targets,
        root_ol: Option<OutputLayout>,
        bcx: &BuildContext,
    ) -> Res<Self> {
        let mut graph = Graph::new(solve.inner.map(|_, _| Job::default(), |_, _| ()));

        let mut curr_layer = HashSet::new();
        let mut next_layer = HashSet::new();

        // We start with the root node.
        next_layer.insert(NodeIndex::new(0));

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
                let build_hash = BuildHash::new(source, &solve, &targets);

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

        Ok(JobQueue { graph, root_ol })
    }

    pub fn exec(mut self, bcx: &BuildContext) -> Res<(Vec<PathBuf>, Vec<(PathBuf, String)>)> {
        let threads = bcx.threads;
        let mut thread_pool = Pool::new(threads);

        let root_ol = &self.root_ol;
        let root_hash = self.graph.inner.raw_nodes().get(0).and_then(|x| {
            if let Work::Dirty(_, h) = &x.weight.work {
                Some(h.clone())
            } else {
                None
            }
        });

        // Bottom jobs are Dirty jobs whose dependencies are all satisfied.
        let bottom_jobs = self.graph.inner.node_indices().filter(|&index| {
            self.graph[index].work.is_dirty() && self
                .graph
                .children(index)
                .all(|(child, _)| self.graph[child].work.is_fresh())
        });

        // this queue only contains dirty jobs.
        let mut queue: Vec<NodeIndex> = Vec::from_iter(bottom_jobs);
        // store job results from threads.
        let done = &MsQueue::new();
        // We also store the locations and summaries of our binaries
        let bins = &MsQueue::new();

        let mut prg = 0;
        let pb = ProgressBar::new(
            self.graph
                .inner
                .node_indices()
                .filter(|&index| self.graph[index].work.is_dirty())
                .count() as u64,
        );
        pb.set_style(ProgressStyle::default_bar().template("  [-->] {bar} {pos}/{len}"));

        loop {
            // break if the job queue is complete
            if queue.is_empty() {
                break;
            }

            // start a group of independent jobs, which can be executed in parallel at current step
            thread_pool.scoped(|scoped| {
                for job_index in queue.drain(..) {
                    if let Work::Dirty(source, build_hash) = &self.graph[job_index].work {
                        let deps = self
                            .graph
                            .children(job_index)
                            .filter(|(_, job)| job.work.is_fresh())
                            .map(|(_, job)| match &job.work {
                                Work::Fresh(binary) => binary,
                                _ => unreachable!(),
                            }).collect::<Vec<_>>();

                        let ts = self.graph[job_index].targets.0.to_vec();

                        pb.println(format!(
                            "{:>7} {} [{}..]",
                            style("[bld]").blue(),
                            source.summary(),
                            &build_hash.0[0..8]
                        ));

                        let pb = &pb;

                        scoped.execute(move || {
                            let op = || -> Res<Option<Binary>> {
                                let tmp;
                                let layout = if job_index == NodeIndex::new(0) {
                                    if let Some(x) = root_ol {
                                        x
                                    } else {
                                        tmp = bcx.cache.checkout_tmp(&build_hash)?;
                                        &tmp
                                    }
                                } else {
                                    tmp = bcx.cache.checkout_tmp(&build_hash)?;
                                    &tmp
                                };

                                let mut res: Option<Binary> = None;

                                for t in ts {
                                    match t {
                                        Target::Lib(cg) => {
                                            let (cmp, cdg) = compile_lib(&source, cg, &deps, &layout, bcx)
                                                .with_context(|e| {
                                                    format!(
                                                        "{:>7} Couldn't build library target for {}\n{}",
                                                        style("[err]").red().bold(),
                                                        source.summary(),
                                                        e
                                                    )
                                                })?;

                                            res = if job_index == NodeIndex::new(0) && root_ol.is_some() {
                                                pb.println(fmt_output(&cmp));
                                                if let Some(cdg) = cdg {
                                                    pb.println(fmt_output(&cdg));
                                                }

                                                let target = DirLock::acquire(&layout.lib)?;
                                                Some(Binary { target })
                                            } else {
                                                Some(
                                                    bcx.cache
                                                        .store_build(&layout.lib, &build_hash)?,
                                                )
                                            }
                                        }
                                        Target::Bin(ix) => {
                                            let (out, path) = compile_bin(
                                                &source,
                                                Target::Bin(ix),
                                                &deps,
                                                &layout,
                                                bcx,
                                            ).with_context(|e| {
                                                format!(
                                                    "{:>7} Couldn't build binary {} for {}\n{}",
                                                    style("[err]").red().bold(),
                                                    ix,
                                                    source.summary(),
                                                    e
                                                )
                                            })?;

                                            bins.push((path, source.summary()));

                                            if job_index == NodeIndex::new(0) && root_ol.is_some() {
                                                pb.println(fmt_output(&out));
                                            }
                                        }
                                        Target::Test(ix) => {
                                            let mut deps = deps.clone();
                                            let root_lib = {
                                                let target =
                                                    DirLock::acquire(&layout.build.join("lib"))?;
                                                Binary { target }
                                            };
                                            deps.push(&root_lib);
                                            let (out, _) = compile_bin(
                                                &source,
                                                Target::Test(ix),
                                                &deps,
                                                &layout,
                                                bcx,
                                            ).with_context(|e| {
                                                format!(
                                                    "{:>7} Couldn't build test {} for {}\n{}",
                                                    style("[err]").red().bold(),
                                                    ix,
                                                    source.summary(),
                                                    e
                                                )
                                            })?;

                                            if job_index == NodeIndex::new(0) && root_ol.is_some() {
                                                pb.println(fmt_output(&out));
                                            }

                                            // For now, only the root package can do tests, so we
                                            // don't worry about storing the binary anywhere.
                                        }
                                        Target::Doc => {
                                            let out = compile_doc(
                                                &source,
                                                &deps,
                                                &layout,
                                                bcx
                                            ).with_context(|e| {
                                                format!(
                                                    "{:>7} Couldn't build docs for {}\n{}",
                                                    style("[err]").red().bold(),
                                                    source.summary(),
                                                    e
                                                )
                                            })?;

                                            if job_index == NodeIndex::new(0) && root_ol.is_some() {
                                                pb.println(fmt_output(&out));
                                            }
                                        }
                                    }
                                }

                                Ok(res)
                            };

                            done.push((job_index, op()));
                        });
                    }
                }
            });

            // Handle the results of job execution
            while let Some((job_index, job_res)) = done.try_pop() {
                match job_res {
                    Ok(binary) => {
                        prg += 1;
                        pb.set_position(prg);

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

                        // push jobs that can be execute at next step into queue
                        for (parent, _) in self.graph.parents(job_index) {
                            let ready = self
                                .graph
                                .children(parent)
                                .all(|(_, job)| job.work.is_fresh());

                            if ready && self.graph[parent].work.is_dirty() {
                                queue.push(parent);
                            }
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
                    }
                    Err(err) => {
                        pb.finish_and_clear();
                        println!("{}", err);
                        bail!("one or more packages couldn't be built")
                    }
                }
            }
        }

        if let Some(ol) = root_ol.as_ref() {
            let res = clear_dir(&ol.build);
            if let Err(e) = res {
                println!(
                    "{:>7} Couldn't clear build directory {}: {}",
                    style("[err]").yellow().bold(),
                    ol.build.display(),
                    e
                );
            }

            if let Some(r) = root_hash {
                let res = ol.write_hash(&r);
                if let Err(e) = res {
                    println!(
                        "{:>7} Couldn't write build hash (root will be rebuilt on next run): {}",
                        style("[err]").yellow().bold(),
                        e
                    );
                }
            }
        }

        let mut bins_vec = vec![];
        while let Some((path, sum)) = bins.try_pop() {
            bins_vec.push((path, sum));
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
            }).collect::<Vec<_>>();

        Ok((root_children, bins_vec))
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
