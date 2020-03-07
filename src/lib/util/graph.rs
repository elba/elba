use crate::util::error::Result;
use petgraph::{
    self,
    graph::NodeIndex,
    visit::{Bfs, EdgeRef, IntoNodeReferences, Walker},
    Direction,
};
use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

/// A wrapper for `petgraph::Graph`.
///
/// This graph is a directed acyclic graph; we always assume that the root node is at node index 0.
#[derive(Debug, Clone)]
pub struct Graph<T>
where
    T: Eq,
{
    pub inner: petgraph::Graph<T, ()>,
}

impl<T: Eq> Graph<T> {
    pub fn new(graph: petgraph::Graph<T, ()>) -> Self {
        Graph { inner: graph }
    }

    pub fn root(&self) -> Option<&T> {
        self.inner.raw_nodes().get(0).map(|node| &node.weight)
    }

    pub fn find_id(&self, node: &T) -> Option<NodeIndex> {
        self.inner
            .node_references()
            .find(|(_, weight)| *weight == node)
            .map(|(index, _)| index)
    }

    pub fn find_by<F>(&self, f: F) -> Option<&T>
    where
        F: Fn(&T) -> bool,
    {
        let node = self.inner.node_references().find(|(_, node)| f(node))?.1;
        Some(node)
    }

    /// Recursively traverse the entire sub tree of the given root, including the root itself
    pub fn sub_tree<'a>(
        &'a self,
        root_id: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &T)> + 'a {
        Bfs::new(&self.inner, root_id)
            .iter(&self.inner)
            .map(move |node_id| (node_id, &self.inner[node_id]))
    }

    /// Traverse all direct children of the given node
    pub fn children<'a>(
        &'a self,
        parent_id: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &T)> + 'a {
        self.inner
            .neighbors_directed(parent_id, Direction::Outgoing)
            .map(move |node_id| (node_id, &self.inner[node_id]))
    }

    /// Traverse all direct parents of the given node
    pub fn parents<'a>(
        &'a self,
        child_id: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &T)> + 'a {
        self.inner
            .neighbors_directed(child_id, Direction::Incoming)
            .map(move |node_id| (node_id, &self.inner[node_id]))
    }

    pub fn map<U, F>(&self, mut f: F) -> Result<Graph<U>>
    where
        U: Eq,
        F: FnMut(NodeIndex, &T) -> Result<U>,
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
            tree.add_edge(node_map[&edge.source()], node_map[&edge.target()], ());
        }

        Ok(Graph::new(tree))
    }
}

impl<T> Index<NodeIndex> for Graph<T>
where
    T: Eq,
{
    type Output = T;

    fn index(&self, index: NodeIndex) -> &T {
        &self.inner[index]
    }
}

impl<T> IndexMut<NodeIndex> for Graph<T>
where
    T: Eq,
{
    fn index_mut(&mut self, index: NodeIndex) -> &mut T {
        &mut self.inner[index]
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
