use package::{
    lockfile::{LockedPkg, LockfileToml},
    resolution::Resolution,
    PackageId, Summary,
};
use petgraph::{
    graph::NodeIndex,
    visit::{Bfs, IntoNodeReferences, Walker},
    Direction, Graph,
};
use retrieve::cache::Source;
use semver::Version;

pub type SourceSolve = Graph<Source, ()>;

/// Represents a fully resolved package dependency graph.
#[derive(Debug, Clone)]
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
        Some(
            Bfs::new(&self.graph, root)
                .iter(&self.graph)
                .map(move |node_id| &self.graph[node_id]),
        )
    }

    pub fn find_node(&self, node: &Summary) -> Option<NodeIndex> {
        self.graph
            .node_references()
            .find(|(_, summary)| *summary == node)
            .map(|(index, _)| index)
    }

    pub fn get_pkg_version(&self, node: &PackageId) -> Option<Version> {
        self.graph
            .node_references()
            .find(|(_, sum)| sum.id() == node)
            .map(|(_, sum)| sum.version().clone())
    }
}

impl Into<LockfileToml> for Solve {
    fn into(self) -> LockfileToml {
        let mut packages = indexset!();

        // Just in case the root node ain't at 0
        let root = self
            .graph
            .node_references()
            .find(|(_, sum)| *sum.resolution() == Resolution::Root)
            .map(|(index, _)| index)
            .unwrap_or_else(|| NodeIndex::new(0));

        let deps = Bfs::new(&self.graph, root).iter(&self.graph);

        for nix in deps {
            let pkg = &self.graph[nix];
            let mut deps_iter = self
                .graph
                .neighbors_directed(nix, Direction::Outgoing)
                .detach();
            let mut this_deps = vec![];

            while let Some(dep) = deps_iter.next_node(&self.graph) {
                this_deps.push(self.graph[dep].clone());
            }

            packages.insert(LockedPkg {
                sum: pkg.clone(),
                dependencies: this_deps,
            });
        }

        LockfileToml { packages }
    }
}

// TODO: verify that this is a valid solve
impl From<LockfileToml> for Solve {
    fn from(f: LockfileToml) -> Self {
        let mut tree = Graph::new();
        let mut set = indexmap!();

        // We don't assume that nix 0 is root here.
        for pkg in f.packages {
            let nix = if set.contains_key(&pkg.sum) {
                set[&pkg.sum]
            } else {
                let nix = tree.add_node(pkg.sum.clone());
                set.insert(pkg.sum, nix);
                nix
            };

            for dep in pkg.dependencies {
                let dep_nix = if set.contains_key(&dep) {
                    set[&dep]
                } else {
                    let nix = tree.add_node(dep.clone());
                    set.insert(dep, nix);
                    nix
                };

                tree.add_edge(nix, dep_nix, ());
            }
        }

        Solve::new(tree)
    }
}

impl Default for Solve {
    fn default() -> Self {
        Solve {
            graph: Graph::new(),
        }
    }
}
