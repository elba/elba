use petgraph::{
    self,
    graph::NodeIndex,
    visit::{Bfs, EdgeRef, IntoNodeReferences, Walker},
    Direction,
};
use std::collections::HashMap;
use util::errors::Res;

#[derive(Debug, Clone)]
pub struct Graph<T>
where
    T: Eq,
{
    inner: petgraph::Graph<T, ()>,
}

impl<T> Graph<T>
where
    T: Eq,
{
    pub fn new(graph: petgraph::Graph<T, ()>) -> Self {
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
    pub fn sub_tree<'a>(&'a self, root: &T) -> Option<impl Iterator<Item = &T> + 'a> {
        let root_id = self.find_id(root)?;
        let iter = Bfs::new(&self.inner, root_id)
            .iter(&self.inner)
            .map(move |node_id| &self.inner[node_id]);
        Some(iter)
    }

    /// Traverse all direct children of the given node
    pub fn children<'a>(&'a self, parent: &T) -> Option<impl Iterator<Item = &T> + 'a> {
        let parent_id = self.find_id(parent)?;
        let iter = self
            .inner
            .neighbors_directed(parent_id, Direction::Outgoing)
            .map(move |node_id| &self.inner[node_id]);
        Some(iter)
    }

    pub fn map<U, F>(&self, mut f: F) -> Res<Graph<U>>
    where
        U: Eq,
        F: FnMut(&T) -> Res<U>,
    {
        let mut tree = petgraph::Graph::new();
        let mut node_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        // First, we add all the nodes into our graph.
        for (idx, weight) in self.inner.node_references() {
            let new_idx = tree.add_node(f(weight)?);
            node_map.insert(idx, new_idx);
        }

        // Then, we add all the edges.
        for edge in self.inner.edge_references() {
            tree.add_edge(node_map[&edge.source()], node_map[&edge.target()], ());
        }

        Ok(Graph::new(tree))
    }

    pub fn filter_map<U, F>(&mut self, mut f: F) -> Graph<U>
    where
        U: Eq,
        F: FnMut(&T) -> Option<U>,
    {
        unimplemented!()
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
