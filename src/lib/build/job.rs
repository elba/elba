use build::context::BuildContext;
use crossbeam::{channel, deque, thread::scope};
use indexmap::IndexMap;
use petgraph::{graph::NodeIndex, Direction};
use retrieve::cache::Binary;
use std::{thread::sleep, time};
use util::{errors::Res, graph::Graph};

// A task is an elba subcommand that should only be available from the current root project. Tasks
// is a list of the Sources of all the tasks needed for this build.
pub fn plan(
    local: bool,
    root_mode: CompileMode,
    tasks: &[NodeIndex],
    bcx: BuildContext,
) -> Res<JobQueue> {
    let oldg = &bcx.resolve;
    let graph = oldg.map(
        |ix, s| {
            Ok(Job {
                source: bcx.cache.checkout_build(
                    s,
                    &oldg,
                    ix == NodeIndex::new(0) && local || tasks.contains(&ix),
                )?,
                compile_mode: if ix == NodeIndex::new(0) {
                    root_mode
                } else if tasks.contains(&ix) {
                    CompileMode::Bin
                } else {
                    CompileMode::Lib
                },
            })
        },
        |_| Ok(()),
    )?;
    // We drop the old context to drop all of the Sources, releasing our lock on them. We
    // don't need them anymore.
    drop(bcx);

    let mut queue = graph
        .sub_tree(&graph.inner[NodeIndex::new(0)])
        .unwrap()
        .filter(|(_, x)| x.source.is_complete())
        .map(|x| x.0)
        .collect::<Vec<_>>();

    queue.reverse();

    Ok(JobQueue { graph, queue })
}

// Our parallel building strategy is to share the JobQueue across workers. A worker gets a Job by
// popping off the end of the queue. When a Job is completed, the Job is mutated so that its source
// is Binary::Built instead of Binary::New. A worker can only start work when the Job's children
// are all Binary::Built.
//
// Alternate strategy: rely on the Cache and a separate "done" list to indicate if a Job is done
// and where the output of that Job is.
#[derive(Default)]
pub struct JobQueue {
    /// The graph of Jobs which need to be done.
    pub graph: Graph<Job>,
    pub queue: Vec<NodeIndex>,
}

impl JobQueue {
    pub fn exec(self) -> Res<()> {
        let thread_count = 1;

        let graph = self.graph;
        let (push, pull) = deque::fifo();
        let (send, recv) = channel::unbounded();

        if self.queue.is_empty() {
            return Ok(());
        }

        let mut status: IndexMap<NodeIndex, Status> = indexmap!();
        let mut ready = true;

        for i in self.queue {
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

        // TODO: The alternate approach would be sharing memory: we share the status and graph
        // structures across threads (with the former being mutably shared). Idk how we'd allow
        // all the threads to push to the deque tho.
        scope(|scope| {
            for _ in 0..thread_count {
                scope.spawn(|| {
                    loop {
                        match pull.steal() {
                            None => {
                                sleep_sec();
                                continue;
                            }
                            Some(Work::Finish) => {
                                break;
                            }
                            Some(Work::More(i, j)) => {
                                match j.exec() {
                                    Err(_) => {
                                        send.send(WorkRes::Error);
                                        // TODO: Print an error or whatever
                                    }
                                    Ok(_) => send.send(WorkRes::Done(i)),
                                }
                            }
                        }
                    }
                });
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

                if WorkRes::Error == ix || status.iter().all(|(_, st)| *st == Status::Done) {
                    for _ in 0..thread_count {
                        // Tell our threads to finish pls
                        push.push(Work::Finish);
                    }
                    break;
                }
            }
        });


        Ok(())
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
// TODO: Include stuff from our BuildContext, because we can never access that thing ever again
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub source: Binary,
    pub compile_mode: CompileMode,
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

/// The general "mode" of what to do
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// Typecheck a target without codegen
    Lib,
    /// Compile and codegen executable(s)
    ///
    /// This subsumes the "Bench" and "Test" modes since those are just compiling and running
    /// executables anyway
    Bin,
    /// Create documentation
    Doc,
}

fn sleep_sec() {
    sleep(time::Duration::from_millis(1000));
}
