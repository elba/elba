use super::{compile_lib, context::BuildContext, Target, Targets};
use crossbeam::queue::MsQueue;
use petgraph::graph::NodeIndex;
use retrieve::cache::OutputLayout;
use retrieve::cache::{Binary, BuildHash, Source};
use scoped_threadpool::Pool;
use std::collections::HashSet;
use std::iter::FromIterator;
use util::{errors::Res, graph::Graph, lock::DirLock};

pub struct JobQueue {
    /// The graph of jobs which need to be done.
    graph: Graph<Job>,
    root_ol: Option<OutputLayout>,
}

// With the current system, there's very little separation between preparing dependencies and
// generating targets. It also tries to unify dealing with the root package. This can lead to some
// awkward scenarios (we have to check NodeIndex::new(0) several times in this code because of
// having to treat the root package differently). However, the benefit of this is that the entire
// Job graph can have arbitrary targets, and in general we reduce code duplication (building the
// root package is still just building another package, after all).
//
// The alternative is to completely separate building the root package and any targets required from
// the JobQueue; the only job (heh) of the JobQueue is to build the *library dependencies* of the
// root package, and it's up to another set of functions to deal with building the root package and
// any necessary targets (i.e. root package binaries, tests, benches, docs, tasks that the root
// needs, etc.). This system is nice in that it clearly distinguishes root packages (we're doing a
// lot of special-casing on NodeIndex::new(0) in JobQueue right now), but it has some drawbacks of
// its own:
//
//   - We have to duplicate some code. Much of this can be mitigated because we're using compile_lib
//     and compile_bin functions that can be shared between the queue and whatever is responsible
//     for the root, but it still feels like duplication of responsibilities.
//
//   - Building global packages (e.g. `elba install`) is less nice: we have to check out the
//     temporary directory for the root package from the Cache manually.
//
//   - If we want to support tasks, that becomes more complicated too.
impl JobQueue {
    pub fn new(
        solve: Graph<Source>,
        root: Targets,
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
                let build_hash = BuildHash::new(source, &solve);

                let targets = if node == NodeIndex::new(0) {
                    root.clone()
                } else {
                    Targets::new(vec![Target::Lib])
                };

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

    // TODO: Return relevant OutputLayouts
    pub fn exec(mut self, bcx: &BuildContext) -> Res<()> {
        // TODO: How many threads do we want?
        let threads = 1;
        let mut thread_pool = Pool::new(threads);

        let root_ol = &self.root_ol;

        // Bottom jobs are Dirty jobs whose dependencies are all satisfied.
        let bottom_jobs = self.graph.inner.node_indices().filter(|&index| {
            self.graph[index].work.is_dirty()
                && self
                    .graph
                    .children(index)
                    .all(|(child, _)| self.graph[child].work.is_fresh())
        });

        // this queue only contains dirty jobs.
        let mut queue: Vec<NodeIndex> = Vec::from_iter(bottom_jobs);
        // store job results from threads.
        let done = &MsQueue::new();

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
                            })
                            .collect::<Vec<_>>();

                        let ts = self.graph[job_index].targets.0.to_vec();

                        scoped.execute(move || {
                            let op = || -> Res<Option<Binary>> {
                                let layout = bcx.cache.checkout_tmp(&build_hash)?;
                                let layout = if job_index == NodeIndex::new(0) {
                                    if let Some(x) = root_ol {
                                        &x.clone()
                                    } else {
                                        &layout
                                    }
                                } else {
                                    &layout
                                };

                                let mut res: Option<Binary> = None;

                                for t in ts {
                                    match t {
                                        Target::Lib => {
                                            compile_lib(&source, &deps, &layout, bcx)?;
                                            res = if job_index != NodeIndex::new(0) {
                                                Some(
                                                    bcx.cache
                                                        .store_build(&layout.lib, &build_hash)?,
                                                )
                                            } else {
                                                let target = DirLock::acquire(&layout.lib)?;
                                                Some(Binary { target })
                                            }
                                        }
                                        Target::Bin(ix) => unimplemented!(),
                                        Target::Test(ix) => {
                                            // TODO: Build a binary with just a dependency on the lib
                                            unimplemented!()
                                        }
                                        Target::Bench(ix) => {
                                            // TODO: Build a binary with just a dependency on the lib
                                            unimplemented!()
                                        }
                                        Target::Doc => unimplemented!(),
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
                        if let Some(b) = binary {
                            // If we got a compiled library out of it, set the
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

                        // If the parents of any of the childs are done, we can set it to None
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
                    // TODO: log error?
                    Err(err) => return Err(err),
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub work: Work,
    pub targets: Targets,
}

impl Default for Job {
    fn default() -> Self {
        Job {
            work: Work::None,
            targets: Targets::new(vec![Target::Lib]),
        }
    }
}

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
