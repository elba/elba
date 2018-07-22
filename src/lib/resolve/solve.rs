use package::{Summary, lockfile::Lockfile};
use petgraph::{Direction, graph::NodeIndex, visit::{Bfs, IntoNodeReferences, Walker}, Graph};
use retrieve::cache::Source;

pub type SourceSolve = Graph<Source, ()>;

/// Represents a fully resolved package dependency graph.
pub struct Solve {
    graph: Graph<Summary, ()>,
}

impl Solve {
    pub fn new(graph: Graph<Summary, ()>) -> Self {
        Solve { graph }
    }

    /// Recursively traverse all dependencies of a given root, with breadth first
    pub fn deps<'a>(&'a self, root: &Summary) -> Option<impl Iterator<Item = &Summary> + 'a> {
        let root = self.find_node(root)?;
        Some(Bfs::new(&self.graph, root)
            .iter(&self.graph)
            .map(move |node_id| &self.graph[node_id]))
    }

    fn find_node(&self, node: &Summary) -> Option<NodeIndex> {
        self.graph
            .node_references()
            .find(|(_, summary)| *summary == node)
            .map(|(index, _)| index)
    }
}

impl Into<Lockfile> for Solve {
    fn into(self) -> Lockfile {
        let mut packages = indexmap!();

        // The root package is always at nix 0.
        let deps = Bfs::new(&self.graph, NodeIndex::new(0))
            .iter(&self.graph);

        for nix in deps {
            let pkg = &self.graph[nix];
            let mut deps_iter = self.graph.neighbors_directed(nix, Direction::Outgoing).detach();
            let mut this_deps = vec![];

            while let Some(dep) = deps_iter.next_node(&self.graph) {
                this_deps.push(self.graph[dep].clone());
            }
            
            packages.insert(pkg.id.clone(), (pkg.version.clone(), this_deps));
        }

        Lockfile { packages }
    }
}