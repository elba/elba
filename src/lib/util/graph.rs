use petgraph::{
    self,
    graph::NodeIndex,
    visit::{Bfs, EdgeRef, IntoNodeReferences, Walker},
    Direction,
};
use std::collections::HashMap;
use util::errors::Res;

/// A wrapper for `petgraph::Graph`.
///
/// This graph is a directed acyclic graph; we always assume that the root node is at node index 0.
#[derive(Debug, Clone)]
pub struct Graph<T, E = ()>
where
    T: Eq,
{
    pub inner: petgraph::Graph<T, E>,
}

impl<T: Eq, E> Graph<T, E> {
    pub fn new(graph: petgraph::Graph<T, E>) -> Self {
        Graph { inner: graph }
    }

    pub fn find_by<F>(&self, f: F) -> Option<&T>
    where
        F: Fn(&T) -> bool,
    {
        let node = self.inner.node_references().find(|(_, node)| f(node))?.1;
        Some(node)
    }

    /// Recursively traverse the entire sub tree of the given root, including the root itself
    pub fn sub_tree<'a>(&'a self, root: &T) -> Option<impl Iterator<Item = (NodeIndex, &T)> + 'a> {
        let root_id = self.find_id(root)?;
        let iter = Bfs::new(&self.inner, root_id)
            .iter(&self.inner)
            .map(move |node_id| (node_id, &self.inner[node_id]));
        Some(iter)
    }

    /// Traverse all direct children of the given node
    pub fn children<'a>(
        &'a self,
        parent: &T,
    ) -> Option<impl Iterator<Item = (NodeIndex, &T)> + 'a> {
        let parent_id = self.find_id(parent)?;
        let iter = self
            .inner
            .neighbors_directed(parent_id, Direction::Outgoing)
            .map(move |node_id| (node_id, &self.inner[node_id]));
        Some(iter)
    }

    pub fn map<U, V, F, G>(&self, mut f: F, mut g: G) -> Res<Graph<U, V>>
    where
        U: Eq,
        F: FnMut(NodeIndex, &T) -> Res<U>,
        G: FnMut(&E) -> Res<V>,
    {
        let mut tree = petgraph::Graph::new();
        let mut node_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        // First, we add all the nodes into our graph.
        for (idx, weight) in self.inner.node_references() {
            let new_idx = tree.add_node(f(idx, weight)?);
            node_map.insert(idx, new_idx);
        }

        // Then, we add all the edges.
        for edge in self.inner.edge_references() {
            tree.add_edge(
                node_map[&edge.source()],
                node_map[&edge.target()],
                g(edge.weight())?,
            );
        }

        Ok(Graph::new(tree))
    }

    pub fn filter_map<U, V, F, G>(&mut self, mut f: F, mut g: G) -> Graph<U, V>
    where
        U: Eq,
        F: FnMut(&T) -> Option<U>,
        G: FnMut(&E) -> Option<V>,
    {
        Graph {
            inner: self.inner.filter_map(|_, i| f(i), |_, j| g(j)),
        }
    }

    fn find_id(&self, node: &T) -> Option<NodeIndex> {
        self.inner
            .node_references()
            .find(|(_, weight)| *weight == node)
            .map(|(index, _)| index)
    }
}

impl<T> Default for Graph<T>
where
    T: Eq,
{
    fn default() -> Self {
        Graph {
            inner: petgraph::Graph::new(),
        }
    }
}
