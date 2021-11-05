use crate::v3::core::node_id::NodeId;
use crate::v3::core::packed_bdd_node::PackedBddNode;

#[derive(Clone)]
pub struct Bdd {
    /// The length of the longest path in the BDD. It is used to compute an upper bound for
    /// various graph manipulation algorithms.
    ///
    /// The algorithms should not assume that this value is exact: it can be an upper bound on
    /// the graph height if the actual height hasn't been computed yet.
    height: usize,
    /// A linearized version of the BDD graph. You should not assume that the nodes have any
    /// specific ordering, aside from the fact that zero/one terminals are the first,
    /// and root node is last.
    nodes: Vec<PackedBddNode>
}

impl Bdd {

    pub fn is_false(&self) -> bool {
        self.nodes.len() == 1
    }

    pub fn is_true(&self) -> bool {
        self.nodes.len() == 2
    }

    pub fn get_root_id(&self) -> NodeId {
        NodeId::from(self.nodes.len() - 1)
    }

    pub unsafe fn get_node_unchecked(&self, id: NodeId) -> &PackedBddNode {
        unsafe { self.nodes.get_unchecked(id.into_usize()) }
    }

    /// Create a new BDD from a vector of nodes without checking that the nodes satisfy
    /// invariants required by the `Bdd` struct.
    pub unsafe fn from_raw_nodes(nodes: Vec<PackedBddNode>) -> Bdd {
        // A reasonable approximation of the true BDD height assuming all invariants are satisfied:
        let height = if nodes.len() <= 2 { 0 } else {
            let last_variable: usize = nodes[2].get_variable().into();
            let first_variable: usize = nodes[nodes.len() - 1].get_variable().into();
            last_variable - first_variable
        };
        Bdd { height, nodes }
    }

}

impl Bdd {

    /// Create a copy of this `Bdd` that is sorted based on the DFS-preorder.
    pub fn sort_preorder(&self) -> Bdd {
        // TODO: This is a reasonable algorithm, but we may want to eliminate bounds checking.

        if self.nodes.len() <= 2 {
            return self.clone();
        }

        let mut id_map = vec![NodeId::UNDEFINED; self.nodes.len()];
        id_map[0] = NodeId::ZERO;
        id_map[1] = NodeId::ONE;

        let mut search_stack: Vec<NodeId> = Vec::with_capacity(self.height);
        search_stack.push(self.get_root_id());

        let mut next_free_id = self.nodes.len() - 1;
        while !search_stack.is_empty() {
            todo!()
        }
        todo!()
    }

}