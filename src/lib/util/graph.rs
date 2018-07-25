use petgraph::{
    self,
    graph::NodeIndex,
    visit::{Bfs, IntoNodeReferences, Walker},
    Direction,
};

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
    pub fn new() -> Self {
        Graph {
            inner: petgraph::Graph::new(),
        }
    }

    pub fn add(&mut self, node: T) {
        self.inner.add_node(node);
    }

    pub fn link(&mut self, parent: &T, child: &T) {
        let parent_id = self.find_id(parent).unwrap();
        let child_id = self.find_id(child).unwrap();
        self.inner.add_edge(parent_id, child_id, ());
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

    /// Traverse all direct childs of the given node
    pub fn childs<'a>(&'a self, parent: &T) -> Option<impl Iterator<Item = &T> + 'a> {
        let parent_id = self.find_id(parent)?;
        let iter = self
            .inner
            .neighbors_directed(parent_id, Direction::Outgoing)
            .map(move |node_id| &self.inner[node_id]);
        Some(iter)
    }

    pub fn map<U, F>(&mut self, mut f: F) -> Graph<U>
    where
        U: Eq,
        F: FnMut(&T) -> U,
    {
        Graph {
            inner: self.inner.map(|_, node| f(node), |_, _| ()),
        }
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
