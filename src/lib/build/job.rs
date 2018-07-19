use build::context::BuildContext;
use failure::Error;
use petgraph::Graph;
use retrieve::cache::Source;

pub fn plan(root: Job, bcx: &BuildContext) -> JobQueue {
    unimplemented!()
}

pub struct JobQueue {
    queue: Graph<Job, ()>,
}

impl JobQueue {
    pub fn exec(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}

// ALl information that defines a compilation task
pub struct Job {
    pub source: Source,
    pub mode: CompileMode,
    pub output: Output,
}

impl Job {
    pub fn exec(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}

/// The general "mode" of what to do
// TODO: Test, Mkdoc, Bench
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// Typecheck a target without codegen
    Lib,
    /// Compile and codegen an executable
    Bin,
}

/// Place to store the output
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum Output {
    // Store the output at source's target dir
    Target,
    // Store the output at global
    Global,
}
