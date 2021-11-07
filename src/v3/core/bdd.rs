use crate::v3::core::node_id::NodeId;
use crate::v3::core::packed_bdd_node::PackedBddNode;
use std::convert::TryFrom;
use crate::v3::core::variable_id::VariableId;

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

    pub fn get_height(&self) -> usize {
        self.height
    }

    pub unsafe fn get_node_unchecked(&self, id: NodeId) -> &PackedBddNode {
        unsafe { self.nodes.get_unchecked(id.into_usize()) }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
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
        if self.nodes.len() <= 2 {  // Skip for trivial BDDs.
            return self.clone();
        }

        let mut id_map = vec![NodeId::UNDEFINED; self.nodes.len()];
        id_map[0] = NodeId::ZERO;
        id_map[1] = NodeId::ONE;

        let mut search_stack: Vec<NodeId> = Vec::with_capacity(self.height);
        search_stack.push(self.get_root_id());

        // Populate `id_map` based on DFS preorder.
        let mut next_free_id = self.nodes.len() - 1;
        while let Some(task) = search_stack.pop() {
            let task_id = unsafe { id_map.get_unchecked_mut(task.into_usize()) };
            if task_id.is_undefined() {
                *task_id = NodeId::from(next_free_id);
                next_free_id -= 1;

                let node = unsafe { self.get_node_unchecked(task) };
                search_stack.push(node.get_high_link());
                search_stack.push(node.get_low_link());
            }
        }

        // Every ID should be assigned except for 0/1.
        assert_eq!(next_free_id, 1);
        unsafe { self.copy_shuffled(&id_map) }
    }

    /// Create a copy of this `Bdd` that is sorted based on the DFS-postorder.
    pub fn sort_postorder(&self) -> Bdd {
        if self.nodes.len() <= 2 {  // Skip for trivial BDDs.
            return self.clone();
        }

        let mut id_map = vec![NodeId::UNDEFINED; self.nodes.len()];
        id_map[0] = NodeId::ZERO;
        id_map[1] = NodeId::ONE;

        let mut search_stack: Vec<(NodeId, bool)> = Vec::with_capacity(self.height);
        search_stack.push((self.get_root_id(), false));

        let mut next_free_id = 2usize;
        while let Some((task, expended)) = search_stack.pop() {
            let task_id = unsafe { id_map.get_unchecked_mut(task.into_usize()) };
            if expended {
                // All children are exported and the task can get its ID now:
                *task_id = NodeId::from(next_free_id);
                next_free_id += 1;
            } else if task_id.is_undefined() {
                // Task is not expanded and the result is so far unknown.
                let node = unsafe { self.get_node_unchecked(task) };
                search_stack.push((task, true));
                search_stack.push((node.get_high_link(), false));
                search_stack.push((node.get_low_link(), false));
            }
        }

        assert_eq!(next_free_id, self.nodes.len());
        unsafe { self.copy_shuffled(&id_map) }
    }

    /// A utility function used by the sort procedures. It takes a shuffle vector (new id for
    /// the node at the respective position) and produces a copy of this ID after performing
    /// the shuffle. The function is unsafe because it does not check whether the shuffle
    /// is actually valid (only contains valid IDs and produces the same BDD).
    ///
    /// The shuffle vector must also correctly place the zero/one nodes (although this assumption
    /// may not be used by the function). Also, this function only works for non-trivial BDDs.
    unsafe fn copy_shuffled(&self, shuffle: &[NodeId]) -> Bdd {
        debug_assert!(shuffle.len() > 2);

        // A trick which avoids unnecessary memory initialization.
        let mut new_nodes = Vec::with_capacity(self.nodes.len());
        unsafe { new_nodes.set_len(self.nodes.len()); }

        // Setup the base
        new_nodes[0] = PackedBddNode::ZERO.clone();
        new_nodes[1] = PackedBddNode::ONE.clone();

        // Remap nodes into the new vector.
        for (old_id, new_id) in shuffle.iter().enumerate().skip(2) {
            let old_node = unsafe { self.nodes.get_unchecked(old_id) };
            let (variable, old_low, old_high) = old_node.unpack();
            let new_low = unsafe { shuffle.get_unchecked(old_low.into_usize()) };
            let new_high = unsafe { shuffle.get_unchecked(old_high.into_usize()) };
            let new_node = PackedBddNode::pack(variable, *new_low, *new_high);
            let new_slot = unsafe { new_nodes.get_unchecked_mut(new_id.into_usize()) };
            *new_slot = new_node;
        }

        Bdd {
            height: self.height,
            nodes: new_nodes,
        }
    }

}


impl TryFrom<&str> for Bdd {
    type Error = String;

    fn try_from(data: &str) -> Result<Self, Self::Error> {
        let mut nodes = Vec::new();
        for node_string in data.split('|').filter(|s| !s.is_empty()) {
            let mut node_items = node_string.split(',');
            let variable = node_items.next();
            let left_pointer = node_items.next();
            let right_pointer = node_items.next();
            if node_items.next().is_some()
                || variable.is_none()
                || left_pointer.is_none()
                || right_pointer.is_none()
            {
                return Err(format!("Unexpected node representation `{}`.", node_string));
            }
            let variable = if let Ok(x) = variable.unwrap().parse::<u32>() {
                VariableId::from(x)
            } else {
                return Err(format!("Invalid variable numeral `{}`.", variable.unwrap()));
            };
            let low_pointer = if let Ok(x) = left_pointer.unwrap().parse::<u64>() {
                NodeId::from(x)
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    left_pointer.unwrap()
                ));
            };
            let high_pointer = if let Ok(x) = right_pointer.unwrap().parse::<u64>() {
                NodeId::from(x)
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    right_pointer.unwrap()
                ));
            };
            nodes.push(PackedBddNode::pack(variable, low_pointer, high_pointer));
        }
        let zero = nodes.get_mut(0).unwrap();
        *zero = PackedBddNode::ZERO;
        if nodes.len() > 1 {
            let one = nodes.get_mut(1).unwrap();
            *one = PackedBddNode::ONE;
        }
        // TODO: We should do some more validation before we designate the result as safe.
        Ok(unsafe { Bdd::from_raw_nodes(nodes) })
    }
}

#[cfg(test)]
mod test {
    use crate::v3::core::bdd::Bdd;
    use std::convert::TryFrom;

    #[test]
    pub fn basic_sorting_test() {
        let bdd = std::fs::read_to_string("bench_inputs/itgr/large-large-large.109.and_not.left.bdd").unwrap();
        let bdd = Bdd::try_from(bdd.as_str()).unwrap();

        // Note that initially, the BDD is in post-order, but sorted from high to low. Our
        // postorder is from low to high, so neither sort actually corresponds to the original
        // file format.

        let preorder = bdd.sort_preorder();
        let postorder = preorder.sort_postorder();

        assert_ne!(bdd.nodes, preorder.nodes);
        assert_ne!(bdd.nodes, postorder.nodes);
        assert_eq!(preorder.nodes, postorder.sort_preorder().nodes);
        assert_eq!(postorder.nodes, preorder.sort_postorder().nodes);
    }

}