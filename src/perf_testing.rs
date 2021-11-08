
pub mod variable_id {

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
    pub struct VariableId(u32);

    impl VariableId {
        pub const UNDEFINED: VariableId = VariableId(u32::MAX);

        pub fn is_undefined(&self) -> bool {
            *self == Self::UNDEFINED
        }
    }

    impl From<u32> for VariableId {
        fn from(value: u32) -> Self {
            VariableId(value)
        }
    }

    impl From<VariableId> for u32 {
        fn from(value: VariableId) -> Self {
            value.0
        }
    }

    impl From<VariableId> for usize {
        fn from(value: VariableId) -> Self {
            value.0 as usize
        }
    }

}

pub mod node_id {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
    pub struct NodeId(u64); // Only 48-bits should be used.

    impl NodeId {
        pub const ZERO: NodeId = NodeId(0);
        pub const ONE: NodeId = NodeId(1);
        pub const UNDEFINED: NodeId = NodeId((1 << 48) - 1);

        pub fn is_undefined(&self) -> bool {
            *self == Self::UNDEFINED
        }
        pub fn is_zero(&self) -> bool {
            *self == Self::ZERO
        }
        pub fn is_one(&self) -> bool {
            *self == Self::ONE
        }
        pub fn is_terminal(&self) -> bool {
            self.is_zero() || self.is_one()
        }
        pub fn into_usize(self) -> usize {
            self.into()
        }
    }

    impl From<u64> for NodeId {
        fn from(value: u64) -> Self {
            NodeId(value)
        }
    }

    impl From<usize> for NodeId {
        fn from(value: usize) -> Self {
            NodeId(value as u64)
        }
    }

    impl From<NodeId> for u64 {
        fn from(value: NodeId) -> Self {
            value.0
        }
    }

    impl From<NodeId> for usize {
        fn from(value: NodeId) -> Self {
            value.0 as usize
        }
    }

}

pub mod packed_bdd_node {
    use super::variable_id::VariableId;
    use super::node_id::NodeId;

    #[derive(Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PackedBddNode(u64, u64);

    impl PackedBddNode {
        const VARIABLE_MASK: u64 = (u16::MAX as u64) << 48;
        const ADDRESS_MASK: u64 = (1 << 48) - 1;
        pub const ZERO: PackedBddNode = PackedBddNode(Self::VARIABLE_MASK, Self::VARIABLE_MASK);

        pub const ONE: PackedBddNode = PackedBddNode(Self::VARIABLE_MASK + 1, Self::VARIABLE_MASK + 1);

        pub fn pack(variable: VariableId, low_link: NodeId, high_link: NodeId) -> PackedBddNode {
            let variable = u64::from(u32::from(variable));
            let packed_low = u64::from(low_link) | (variable << 48);    // add low 16 bits
            let packed_high = u64::from(high_link) | ((variable << 32) & Self::VARIABLE_MASK);  // add high 16 bits
            PackedBddNode(packed_low, packed_high)
        }

        pub fn unpack(&self) -> (VariableId, NodeId, NodeId) {
            let variable = ((self.0 >> 48) | ((self.0 & Self::VARIABLE_MASK) >> 32)) as u32;
            (VariableId::from(variable), NodeId::from(self.0 & Self::ADDRESS_MASK), NodeId::from(self.1 & Self::ADDRESS_MASK))
        }

        pub fn get_variable(&self) -> VariableId {
            let variable = ((self.0 >> 48) | ((self.0 & Self::VARIABLE_MASK) >> 32)) as u32;
            VariableId::from(variable)
        }

        pub fn get_low_link(&self) -> NodeId {
            NodeId::from(self.0 & Self::ADDRESS_MASK)
        }

        pub fn get_high_link(&self) -> NodeId {
            NodeId::from(self.1 & Self::ADDRESS_MASK)
        }

    }

}

pub mod bdd {
    use super::packed_bdd_node::PackedBddNode;
    use super::node_id::NodeId;
    use super::variable_id::VariableId;
    use std::convert::TryFrom;

    #[derive(Clone)]
    pub struct Bdd {
        height: usize,
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

        pub fn prefetch(&self, id: NodeId) {
            unsafe {
                let pointer: *const PackedBddNode = self.nodes.get_unchecked(id.into_usize());
                std::arch::x86_64::_mm_prefetch::<3>(pointer as *const i8);
            }
        }

        pub fn node_count(&self) -> usize {
            self.nodes.len()
        }

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
        use super::Bdd;
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
}

pub mod bdd_dfs {
    use super::bdd::Bdd;
    use super::node_id::NodeId;

    pub struct UnsafeStack<T: Sized + Copy> {
        index_after_last: usize,
        items: Vec<T>
    }

    impl <T: Sized + Copy> UnsafeStack<T> {

        pub fn new(capacity: usize) -> UnsafeStack<T> {
            let mut items = Vec::with_capacity(capacity);
            unsafe { items.set_len(capacity); }
            UnsafeStack {
                items, index_after_last: 0,
            }
        }

        pub fn is_empty(&self) -> bool {
            self.index_after_last == 0
        }

        pub fn push(&mut self, item: T) {
            let slot = unsafe { self.items.get_unchecked_mut(self.index_after_last) };
            *slot = item;
            self.index_after_last += 1;
        }

        pub fn pop(&mut self) -> T {
            self.index_after_last -= 1;
            unsafe { *self.items.get_unchecked(self.index_after_last) }
        }

    }

    /// A simple function for testing performance of BDD traversal.
    pub fn dfs_node_count(bdd: &Bdd) -> usize {
        let mut count = 0;
        let mut stack = UnsafeStack::<NodeId>::new(bdd.get_height() + 1);
        let mut expanded = vec![false; bdd.node_count()];

        stack.push(bdd.get_root_id());

        while !stack.is_empty() {
            let top = stack.pop();
            let is_expanded = unsafe { expanded.get_unchecked_mut(top.into_usize()) };
            if !*is_expanded {
                *is_expanded = true;
                count += 1;
                if !top.is_terminal() {
                    let node = unsafe { bdd.get_node_unchecked(top) };
                    stack.push(node.get_high_link());
                    stack.push(node.get_low_link());
                }
            }
        }

        count
    }
}

pub mod coupled_dfs {
    use super::bdd::Bdd;
    use super::node_id::NodeId;
    use std::cmp::min;
    use std::num::NonZeroU64;
    use std::ops::{Rem, BitXor};

    struct Cache {
        capacity: NonZeroU64,
        items: Vec<(NodeId, NodeId)>
    }

    impl Cache {
        pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        const HASH_BLOCK: u64 = 1 << 14;

        pub fn new(capacity: usize) -> Cache {
            Cache {
                capacity: unsafe { NonZeroU64::new_unchecked(capacity as u64) },
                items: vec![(NodeId::ZERO, NodeId::ZERO); capacity]
            }
        }

        /// Returns true if task was freshly added and false if it was already in the cache.
        pub fn ensure(&mut self, task: (NodeId, NodeId)) -> bool {
            let slot = self.hashed_index(task);
            let slot_value = unsafe { self.items.get_unchecked_mut(slot ) };
            if *slot_value == task {
                false
            } else {
                *slot_value = task;
                true
            }
        }

        // Locality sensitive hashing algorithm, assuming that left nodes are a
        // mostly-growing sequence.
        fn hashed_index(&self, task: (NodeId, NodeId)) -> usize {
            // The rotation ensures that we don't get an obvious collision when left == right.
            let left_hash = u64::from(task.0).rotate_left(7).wrapping_mul(Self::SEED);
            let right_hash = u64::from(task.1).wrapping_mul(Self::SEED);
            let block_index: u64 = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
            let block_start: u64 = u64::from(task.0);
            (block_start + block_index).rem(self.capacity) as usize
        }

    }

    pub fn coupled_dfs(left_bdd: &Bdd, right_bdd: &Bdd) -> usize {
        let max_height = left_bdd.get_height() + right_bdd.get_height();
        let mut stack: Vec<(NodeId, NodeId)> = Vec::with_capacity(max_height);
        let mut visited = Cache::new(left_bdd.node_count());
        let mut count = 0;

        stack.push((left_bdd.get_root_id(), right_bdd.get_root_id()));
        while let Some((left, right)) = stack.pop() {
            if visited.ensure((left, right)) {
                count += 1;
                if !(left.is_terminal() && right.is_terminal()) {
                    let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                    let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                    let (l_var, l_low, l_high) = left_node.unpack();
                    let (r_var, r_low, r_high) = right_node.unpack();

                    let variable = min(l_var, r_var);

                    let (l_low, l_high) = if l_var == variable {
                        (l_low, l_high)
                    } else {
                        (left, left)
                    };

                    let (r_low, r_high) = if r_var == variable {
                        (r_low, r_high)
                    } else {
                        (right, right)
                    };

                    stack.push((l_high, r_high));
                    stack.push((l_low, r_low));
                }
            }
        }

        count
    }
}