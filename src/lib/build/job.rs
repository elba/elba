use build::context::BuildContext;
use failure::Error;
use petgraph::graph::NodeIndex;
use retrieve::cache::Binary;
use std::collections::VecDeque;
use util::{errors::Res, graph::Graph};

// A task is an elba subcommand that should only be available from the current root project. Tasks
// is a list of the Sources of all the tasks needed for this build.
pub fn plan(root_mode: CompileMode, tasks: &[NodeIndex], bcx: BuildContext) -> Res<JobQueue> {
    let oldg = &bcx.resolve;
    let graph = oldg.map(
        |ix, s| {
            Ok(Job {
                source: bcx.cache.checkout_build(
                    s,
                    &oldg,
                    ix == NodeIndex::new(0) || tasks.contains(&ix),
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

    let queue = graph
        .sub_tree(&graph.inner[NodeIndex::new(0)])
        .unwrap()
        .filter(|(_, x)| {
            if let Binary::New(_, _) = x.source {
                true
            } else {
                false
            }
        })
        .map(|x| x.0)
        .collect::<VecDeque<_>>();

    Ok(JobQueue { graph, queue })
}

// Our parallel building strategy is to share the JobQueue across workers. A worker gets a Job by
// popping off the end of the queue. When a Job is completed, the Job is mutated so that its source
// is Binary::Built instead of Binary::New. A worker can only start work when the Job's children
// are all Binary::Built.
//
// Alternate strategy: rely on the Cache and a separate "done" list to indicate if a Job is done
// and where the output of that Job is.
pub struct JobQueue {
    /// The graph of Jobs which need to be done.
    graph: Graph<Job>,
    /// A queue of indices to Jobs which need to be built. The queue should be popped from the end.
    queue: VecDeque<NodeIndex>,
}

impl JobQueue {
    pub fn exec(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}

/// All the information that defines a compilation task
// TODO: Include stuff from our BuildContext, because we can never access that thing ever again
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub source: Binary,
    pub compile_mode: CompileMode,
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
