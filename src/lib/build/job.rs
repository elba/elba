use super::{CompileMode, compile_lib, context::BuildContext};
use crossbeam::queue::MsQueue;
use package::manifest::BinTarget;
use petgraph::graph::NodeIndex;
use retrieve::cache::{Binary, BuildHash, Source};
use scoped_threadpool::Pool;
use std::collections::HashSet;
use std::iter::FromIterator;
use util::{errors::Res, graph::Graph};

pub struct JobQueue {
    /// The graph of jobs which need to be done.
    graph: Graph<Job>,
}

impl JobQueue {
    // We're using Graph<Job> to move dependency preparation and target generation closer.
    // Executing the JobQueue creates all of the binaries and stuff. Another function will be
    // responsible for any moving around that has to take place.
    pub fn new(solve: Graph<Source>, bcx: &BuildContext) -> Res<Self> {
        let mut graph = Graph::new(solve.inner.map(|_, _| Job::default(), |_, _| ()));

        let mut curr_layer = HashSet::new();
        let mut next_layer = HashSet::new();

        let direct_deps = solve.children(NodeIndex::new(0)).map(|(index, _)| index);

        next_layer.extend(direct_deps);

        while !next_layer.is_empty() {
            debug_assert!(curr_layer.is_empty());

            curr_layer.extend(next_layer.drain());

            for node in curr_layer.drain() {
                let source = &solve[node];
                let build_hash = BuildHash::new(source, &solve);

                let job = match bcx.cache.checkout_build(&build_hash)? {
                    Some(binary) => Job { work: Work::Fresh(binary), compile_mode: CompileMode::Lib, bin_paths: vec![] },
                    None => {
                        next_layer.extend(
                            graph
                                .children(node)
                                .filter(|(_, child)| child.is_none())
                                .map(|(index, _)| index),
                        );

                        Job { work: Work::Dirty(source.clone(), build_hash), compile_mode: CompileMode::Lib, bin_paths: vec![] }
                    }
                };
                graph[node] = job;
            }
        }

        // We drop the all of the Sources, releasing our lock on them. We don't need them anymore.
        // TODO: We may drop Binary as well when all of it's parents are built?
        drop(solve);

        Ok(JobQueue { graph })
    }

    pub fn exec(mut self, bcx: &BuildContext) -> Res<()> {
        // TODO: How many threads do we want?
        let threads = 1;
        let mut thread_pool = Pool::new(threads);

        // Bottom jobs are Dirty jobs whose dependencies are all satisfied.
        let bottom_jobs = self.graph.inner.node_indices().filter(|&index| {
            self.graph[index].is_dirty()
                && self
                    .graph
                    .children(index)
                    .all(|(child, _)| self.graph[child].is_fresh())
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
                            .map(|(_, job)| match &job.work {
                                Work::Fresh(binary) => binary,
                                _ => unreachable!(),
                            })
                            .collect::<Vec<_>>();

                        scoped.execute(move || {
                            let op = || -> Res<Binary> {
                                let layout = bcx.cache.checkout_tmp(&build_hash)?;

                                compile_lib(&source, &deps, &layout, bcx)?;

                                let binary = bcx.cache.store_build(&layout.lib, &build_hash)?;

                                Ok(binary)
                            };

                            done.push((job_index, op()));
                        });
                    }
                }
            });

            // handle the results from job executions
            while let Some((job_index, job_res)) = done.try_pop() {
                match job_res {
                    Ok(binary) => {
                        self.graph[job_index].work = Work::Fresh(binary);

                        // push jobs that can be execute at next step into queue
                        for (parent, _) in self.graph.parents(job_index) {
                            let ready = self.graph.children(parent).all(|(_, job)| job.is_fresh());

                            if ready && self.graph[parent].is_dirty() {
                                queue.push(parent);
                            }
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
    pub compile_mode: CompileMode,
    pub bin_paths: Vec<BinTarget>,
}

impl Default for Job {
    fn default() -> Self {
        Job {
            work: Work::None,
            compile_mode: CompileMode::Lib,
            bin_paths: vec![],
        }
    }
}

impl Job {
    pub fn is_none(&self) -> bool {
        self.work.is_none()
    }

    pub fn is_dirty(&self) -> bool {
        self.work.is_dirty()
    }

    pub fn is_fresh(&self) -> bool {
        self.work.is_fresh()
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
