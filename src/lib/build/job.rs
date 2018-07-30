use super::CompileMode;
use build::context::BuildContext;
use crossbeam::{channel, deque, thread::scope};
use indexmap::IndexMap;
use package::manifest::BinTarget;
use petgraph::{graph::NodeIndex, Direction};
use retrieve::cache::{Binary, Cache, Source};
use std::{thread::sleep, time};
use util::{errors::Res, graph::Graph};

// Our parallel building strategy uses a shared work deque and message passing between the main
// thread and the child threads. The main thread pushes work to the deque when it's available.
// If an error occurs on any of the threads, the main thread will tell all the children to die,
// and return an error overall.
//
// Once all of the dependencies of a package have been built, that package is immediately pushed
// onto the work queue; in other words, we don't spend any time waiting for a whole "layer" of
// packages to be finished, since that can block packages from being built for no reason.
#[derive(Default)]
pub struct JobQueue {
    /// The graph of Jobs which need to be done.
    pub graph: Graph<Job>,
}

impl JobQueue {
    // A task is an elba subcommand that should only be available from the current root project. Tasks
    // is a list of the Sources of all the tasks needed for this build.
    // This function takes another Cache as an argument because that Cache corresponds to the Cache
    // where the root package should be built.
    pub fn new(
        local: &Cache,
        root_mode: CompileMode,
        root_bins: &[BinTarget],
        tasks: &[NodeIndex],
        bcx: BuildContext,
        graph: Graph<Source>,
    ) -> Res<JobQueue> {
        let oldg = graph;
        let graph = oldg.map(
            |ix, s| {
                let src = if ix == NodeIndex::new(0) || tasks.contains(&ix) {
                    local.checkout_build(s, &oldg)?
                } else {
                    bcx.cache.checkout_build(s, &oldg)?
                };

                Ok(Job {
                    compile_mode: if ix == NodeIndex::new(0) {
                        root_mode
                    } else if tasks.contains(&ix) {
                        CompileMode::Bin
                    } else {
                        CompileMode::Lib
                    },
                    bin_paths: if ix == NodeIndex::new(0) {
                        root_bins.to_vec()
                    } else if tasks.contains(&ix) {
                        src.source()
                            .map(|s| s.meta.targets.bin.clone())
                            .unwrap_or_else(|| vec![])
                    } else {
                        vec![]
                    },
                    source: src,
                })
            },
            |_| Ok(()),
        )?;

        // We drop the old context to drop all of the old Sources, releasing our lock on them. We
        // don't need them anymore.
        drop(bcx);

        Ok(JobQueue { graph })
    }

    pub fn exec(self) -> Res<()> {
        let thread_count = 1;

        let graph = self.graph;
        let (push, pull) = deque::fifo();
        let (send, recv) = channel::unbounded();

        if graph.inner.raw_nodes().is_empty() {
            return Ok(());
        }

        let mut queue = graph
            .sub_tree(NodeIndex::new(0))
            .filter(|(_, x)| x.source.is_complete())
            .map(|x| x.0)
            .collect::<Vec<_>>();

        queue.reverse();

        if queue.is_empty() {
            return Ok(());
        }

        let mut status: IndexMap<NodeIndex, Status> = indexmap!();
        let mut ready = true;

        for i in queue {
            ready = ready
                && graph
                    .inner
                    .neighbors_directed(i, Direction::Outgoing)
                    .all(|node_id| graph.inner[node_id].is_done());

            if !ready {
                status.insert(i, Status::Waiting);
            } else {
                status.insert(i, Status::Queued);
                push.push(Work::More(i, &graph.inner[i]));
            }
        }

        scope(|scope| -> Res<()> {
            let mut threads = vec![];
            for _ in 0..thread_count {
                threads.push(scope.spawn(|| -> Res<()> {
                    loop {
                        match pull.steal() {
                            None => {
                                sleep_sec();
                                continue;
                            }
                            Some(Work::Finish) => {
                                break;
                            }
                            Some(Work::More(i, j)) => match j.exec() {
                                Err(e) => {
                                    send.send(WorkRes::Error);
                                    return Err(e);
                                }
                                Ok(_) => send.send(WorkRes::Done(i)),
                            },
                        }
                    }

                    Ok(())
                }));
            }

            // Here we coordinate between our channels and such
            loop {
                // It shouldn't close on us... ever.
                let ix = recv.recv().unwrap();
                if let WorkRes::Done(ix) = ix {
                    status[&ix] = Status::Done;
                    for n in graph.inner.neighbors_directed(ix, Direction::Incoming) {
                        // `status` might not contain n, since it only contains nodes which weren't
                        // built before our build process started.
                        if let Some(Status::Waiting) = status.get(&n) {
                            let ready = graph
                                .inner
                                .neighbors_directed(n, Direction::Outgoing)
                                .all(|node_id| status[&node_id] == Status::Done);

                            if ready {
                                push.push(Work::More(n, &graph.inner[n]));
                                status[&n] = Status::Queued;
                            }
                        }
                    }
                }

                if WorkRes::Error == ix {
                    for _ in 0..thread_count {
                        // Tell our threads to finish pls
                        push.push(Work::Finish);
                    }
                    for t in threads {
                        t.join().unwrap()?;
                    }

                    // At least one of our threads has to have errored at this point
                    unreachable!()
                }

                if status.iter().all(|(_, st)| *st == Status::Done) {
                    for _ in 0..thread_count {
                        // Tell our threads to finish pls
                        push.push(Work::Finish);
                    }
                    break;
                }
            }

            Ok(())
        })
    }
}

#[derive(Debug)]
enum Work<'a> {
    More(NodeIndex, &'a Job),
    Finish,
}

#[derive(Debug, PartialEq, Eq)]
enum WorkRes {
    Done(NodeIndex),
    Error,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Status {
    Done,
    Queued,
    Waiting,
}

/// All the information that defines a compilation task
// TODO: How do we pass the correct Layout to this Job
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub source: Binary,
    pub compile_mode: CompileMode,
    pub bin_paths: Vec<BinTarget>,
}

impl Job {
    pub fn is_done(&self) -> bool {
        self.source.is_complete()
    }

    pub fn exec(&self) -> Res<()> {
        // TODO
        println!("From one to ten, this is very unimplemented.");
        Ok(())
    }
}

fn sleep_sec() {
    sleep(time::Duration::from_millis(1000));
}
