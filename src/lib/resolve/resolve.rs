use package::Summary;
use petgraph::graph::NodeIndex;
use petgraph::visit::{Bfs, IntoNodeReferences, Visitable, Walker, WalkerIter};
use petgraph::Graph;

/// Represents a fully resolved package dependency graph.
pub struct Resolve {
    graph: Graph<Summary, ()>,
}

impl Resolve {
    pub fn new(graph: Graph<Summary, ()>) -> Self {
        Resolve { graph }
    }

    /// Recursively traverse all dependencies of a given root, with breadth first
    pub fn deps<'a>(&'a self, root: &Summary) -> impl Iterator<Item = &Summary> + 'a {
        let root = self.find_node(root);
        Bfs::new(&self.graph, root)
            .iter(&self.graph)
            .map(move |node_id| &self.graph[node_id])
    }

    fn find_node(&self, node: &Summary) -> NodeIndex {
        self.graph
            .node_references()
            .find(|(_, summary)| *summary == node)
            .map(|(index, _)| index)
            .expect("the node is not in dependency tree")
    }
}
