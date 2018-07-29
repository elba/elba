use build::compile_lib;
use build::context::{BuildContext, Layout};
use crossbeam::queue::MsQueue;
use petgraph::graph::NodeIndex;
use retrieve::{Binary, BuildHash, Source};
use scoped_threadpool::Pool;
use std::collections::HashSet;
use std::iter::FromIterator;
use util::{errors::Res, graph::Graph};

/// JobQueue is responsible for building direct dependencies for root package.
///
/// JobQueue transforms dirty jobs into fresh jobs, starting from the bottom leaves.
/// A built job is called `Fresh` or else it is `Dirty`. Job of `None` is either root
/// or a package that is unreachable from root(the intermediate packages were built).
pub struct JobQueue {
    /// The graph of Jobs which need to be done.
    graph: Graph<Job>,
}

impl JobQueue {
    pub fn new(solve: Graph<Source>, bcx: &BuildContext) -> Res<Self> {
        let mut graph = solve.map(|_, _| Ok(Job::None), |_| Ok(()))?;

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
                    Some(binary) => Job::Fresh(binary),
                    None => {
                        next_layer.extend(
                            graph
                                .children(node)
                                .filter(|(_, child)| child.is_none())
                                .map(|(index, _)| index),
                        );

                        Job::Dirty(source.clone(), build_hash)
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

    pub fn exec(mut self, bcx: &BuildContext) -> Res<Vec<Binary>> {
        let mut thread_pool = Pool::new(1);

        // bottom job is dirty job that has no deps or has all deps built
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
                    if let Job::Dirty(source, build_hash) = &self.graph[job_index] {
                        let deps = self
                            .graph
                            .children(job_index)
                            .map(|(_, job)| match job {
                                Job::Fresh(binary) => binary.clone(),
                                _ => unreachable!(),
                            })
                            .collect();

                        scoped.execute(move || {
                            let op = || -> Res<Binary> {
                                let dl = bcx.cache.acquire_build_tempdir(&build_hash)?;
                                let layout = Layout::new(dl)?;

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
                        self.graph[job_index] = Job::Fresh(binary);

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

        let deps = self
            .graph
            .children(NodeIndex::new(0))
            .map(|(_, job)| match job {
                Job::Fresh(binary) => binary.clone(),
                _ => unreachable!(),
            })
            .collect();

        Ok(deps)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Job {
    None,
    Fresh(Binary),
    Dirty(Source, BuildHash),
}

impl Job {
    pub fn is_none(&self) -> bool {
        match self {
            Job::None => true,
            _ => false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            Job::Dirty(_, _) => true,
            _ => false,
        }
    }

    pub fn is_fresh(&self) -> bool {
        match self {
            Job::Fresh(_) => true,
            _ => false,
        }
    }
}
