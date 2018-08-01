use super::{compile_lib, context::BuildContext, Target, Targets};
use crossbeam::queue::MsQueue;
use petgraph::graph::NodeIndex;
use retrieve::cache::OutputLayout;
use retrieve::cache::{Binary, BuildHash, Source};
use scoped_threadpool::Pool;
use std::collections::HashSet;
use std::iter::FromIterator;
use util::{errors::Res, graph::Graph};

pub struct JobQueue {
    /// The graph of jobs which need to be done.
    graph: Graph<Job>,
}

// TODO: The main deficiency with the current system is in how we construct the Job graph:
// specifically, how we check if a build is complete. Because we always check the global
// cache for if a build folder for the package exists, there are several consequences:
//
//   - The root package will be unconditionally rebuilt every time, because its output
//     only exists in the root directory.
//     The solution to this is that we could deal with this by passing the root_ol
//     into JobQueue::new instead of JobQueue::exec so that we can check the hash of the root
//     Source from the graph versus the hash stored in the OutputLayout (we should add a hash to
//     the OutputLayout too to store the previous hash of the source built in it). Maybe we should
//     store the root OutputLayout as part of the struct?
//
//   - If a non-root package has multiple targets (say, a lib and 2 bins), but it's corrupted
//     midway through the process, the system might be fooled into thinking that the Job is
//     complete when it isn't (i.e. if it successfully builds the lib but fails in the bin).
//     The solution to this really is just don't build targets in parallel.
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
//   - Building global packages (e.g. `elba install`) is less nice: we would have to make a
//     temporary directory (as in somewhere in `/tmp/`) to replace what would be `./target/`,
//     or we have to check out the temporary directory for the root package from the Cache
//     manually.
//
//   - If we want to support tasks, that becomes more complicated too.
impl JobQueue {
    // We're using Graph<Job> to move dependency preparation and target generation closer.
    // Executing the JobQueue creates all of the binaries and stuff. Another function will be
    // responsible for any moving around that has to take place.
    pub fn new(
        solve: Graph<Source>,
        root: Targets,
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

                let job = match bcx.cache.checkout_build(&build_hash)? {
                    Some(binary) => Job {
                        work: Work::Fresh(binary),
                        targets
                    },
                    None => {
                        next_layer.extend(
                            graph
                                .children(node)
                                // If the Job is none, that means that it hasn't been visited yet.
                                .filter(|(_, child)| child.is_none())
                                .map(|(index, _)| index),
                        );

                        Job {
                            work: Work::Dirty(source.clone(), build_hash),
                            targets
                        }
                    }
                };
                graph[node] = job;
            }
        }

        // We drop the all of the Sources, releasing our lock on them. We don't need them anymore.
        drop(solve);

        Ok(JobQueue { graph })
    }

    // TODO: Return relevant OutputLayouts
    pub fn exec(mut self, bcx: &BuildContext, root_ol: &Option<OutputLayout>) -> Res<()> {
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
                                    if let Some(x) = &root_ol {
                                        x
                                    } else {
                                        &layout
                                    }
                                } else {
                                    &layout
                                };
                                
                                let mut res: Option<Binary> = None;

                                // TODO: In order to build tasks in parallel, for everything except lib we could
                                // spawn another task to execute
                                // e.g. Target::Bin(ix) => { scoped.execute(move || ...) }
                                for t in ts {
                                    match t {
                                        Target::Lib => {
                                            compile_lib(&source, &deps, &layout, bcx)?;
                                            if job_index != NodeIndex::new(0) {
                                                res = Some(bcx.cache.store_build(&layout.lib, &build_hash)?);
                                            }
                                        }
                                        Target::Bin(ix) => {
                                            unimplemented!()
                                        }
                                        Target::Test(ix) => {
                                            // TODO: Build a binary with just a dependency on the lib
                                            unimplemented!()
                                        }
                                        Target::Bench(ix) => {
                                            // TODO: Build a binary with just a dependency on the lib
                                            unimplemented!()
                                        }
                                        Target::Doc => {
                                            unimplemented!()
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
                        
                        let mut parents_done = true;

                        // push jobs that can be execute at next step into queue
                        for (parent, _) in self.graph.parents(job_index) {
                            parents_done &= !self.graph[parent].is_dirty();
                            let ready = self.graph.children(parent).all(|(_, job)| job.is_fresh());

                            if ready && self.graph[parent].is_dirty() {
                                queue.push(parent);
                            }
                        }
                        
                        // If all of the parents of this dep are done, we can set ourselves to None
                        if parents_done {
                            self.graph[job_index].work = Work::None;
                        }
                    }
                    // TODO: log error?
                    // TODO: If the `~/.elba/build/hash` dir was created, remove it
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
