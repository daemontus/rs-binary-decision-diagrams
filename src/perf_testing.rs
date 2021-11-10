
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
        pub const UNDEFINED: NodeId = NodeId(u64::MAX);

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
    pub struct PackedBddNode(u32, u64, u64);

    impl PackedBddNode {
        pub const ZERO: PackedBddNode = PackedBddNode(u32::MAX, 0, 0);

        pub const ONE: PackedBddNode = PackedBddNode(u32::MAX, 0 , 0);

        pub fn pack(variable: VariableId, low_link: NodeId, high_link: NodeId) -> PackedBddNode {
            PackedBddNode(u32::from(variable), u64::from(low_link), u64::from(high_link))
        }

        pub fn unpack(&self) -> (VariableId, NodeId, NodeId) {
            (VariableId::from(self.0), NodeId::from(self.1), NodeId::from(self.2))
        }

        pub fn get_variable(&self) -> VariableId {
            VariableId::from(self.0)
        }

        pub fn get_low_link(&self) -> NodeId {
            NodeId::from(self.1)
        }

        pub fn get_high_link(&self) -> NodeId {
            NodeId::from(self.2)
        }

        pub fn eq_variable(&self, variable: VariableId) -> bool {
            self.0 == variable.into()
        }

        pub fn eq_low(&self, low: NodeId) -> bool {
            self.1 == low.into()
        }

        pub fn eq_high(&self, high: NodeId) -> bool {
            self.2 == high.into()
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

    pub struct PointerUnsafeStack<T: Sized + Copy> {
        pointer_last: *mut T,
        pointer_zero: *mut T,
        _items: Vec<T>,
    }

    impl <T: Sized + Copy> PointerUnsafeStack<T> {

        pub fn new(capacity: usize) -> PointerUnsafeStack<T> {
            // This design sacrifices the first element as a "stopper", but requires less
            // pointer arithmetic co implement correctly.
            let mut data = Vec::with_capacity(capacity + 1);
            unsafe {
                data.set_len(capacity + 1);
                let base_pointer: *mut T = data.get_unchecked_mut(0);
                PointerUnsafeStack {
                    pointer_last: base_pointer,
                    pointer_zero: base_pointer,
                    _items: data
                }
            }
        }

        pub fn is_empty(&self) -> bool {
            self.pointer_zero == self.pointer_last
        }

        pub fn len(&self) -> usize {
            unsafe {
                self.pointer_last.offset_from(self.pointer_zero).abs() as usize
            }
        }

        pub fn peek(&mut self) -> &mut T {
            unsafe {
                &mut *self.pointer_last
            }
        }

        pub fn peek_at(&mut self, offset: usize) -> &mut T {
            unsafe {
                &mut *self.pointer_last.sub(offset)
            }
        }

        pub fn push(&mut self, item: T) {
            unsafe {
                self.pointer_last = self.pointer_last.add(1);
                *self.pointer_last = item;
            }
        }

        pub fn pop(&mut self) {
            unsafe {
                self.pointer_last = self.pointer_last.sub(1);
            }
        }

    }

    impl <T: Sized + Copy> UnsafeStack<T> {

        pub fn new(capacity: usize) -> UnsafeStack<T> {
            let mut items = Vec::with_capacity(capacity);
            unsafe { items.set_len(capacity); }
            UnsafeStack {
                items, index_after_last: 0,
            }
        }

        pub fn len(&self) -> usize {
            self.index_after_last
        }

        pub fn is_empty(&self) -> bool {
            self.index_after_last == 0
        }

        pub fn peek(&mut self ) -> &mut T {
            unsafe { self.items.get_unchecked_mut(self.index_after_last - 1) }
        }

        pub fn peek_at(&mut self, offset: usize) -> &mut T {
            unsafe { self.items.get_unchecked_mut(self.index_after_last - offset) }
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
    use std::num::NonZeroU64;
    use std::ops::Rem;
    use super::bdd_dfs::UnsafeStack;

    struct Cache {
        capacity: NonZeroU64,
        items: Vec<(NodeId, NodeId)>
    }

    impl Cache {
        pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        const HASH_BLOCK: u64 = 1 << 13;

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
            let right_hash = u64::from(task.1).wrapping_mul(Self::SEED);
            let block_index = right_hash.rem(Self::HASH_BLOCK);
            let block_start: u64 = u64::from(task.0);

            unsafe {
                // Usually not that important, but seems to be actually helping for large BDDs.
                let pointer: *const (NodeId, NodeId) =
                    self.items.get_unchecked((block_start as usize) + 128);
                std::arch::x86_64::_mm_prefetch::<1>(pointer as *const i8);
            }
            (block_start + block_index).rem(self.capacity) as usize
        }

    }

    pub fn coupled_dfs(left_bdd: &Bdd, right_bdd: &Bdd) -> usize {
        let height_sum = left_bdd.get_height() + right_bdd.get_height();
        let mut stack: UnsafeStack<(NodeId, NodeId)> = UnsafeStack::new(height_sum);
        let mut visited = Cache::new(left_bdd.node_count());
        let mut count = 0;

        stack.push((left_bdd.get_root_id(), right_bdd.get_root_id()));
        while !stack.is_empty() {
            let (left, right) = stack.pop();
            if visited.ensure((left, right)) {
                count += 1;

                let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                let (l_var, l_low, l_high) = left_node.unpack();
                let (r_var, r_low, r_high) = right_node.unpack();

                // This explicit "switch" is slightly faster. Not sure exactly why, but
                // it is probably easier to branch predict.
                if l_var == r_var {
                    if !(l_high.is_terminal() && r_high.is_terminal()) {
                        stack.push((l_high, r_high));
                    }
                    if !(l_low.is_terminal() && r_low.is_terminal()) {
                        stack.push((l_low, r_low));
                    }
                } else if l_var < r_var {
                    if !(l_high.is_terminal() && right.is_terminal()) {
                        stack.push((l_high, right));
                    }
                    if !(l_low.is_terminal() && right.is_terminal()) {
                        stack.push((l_low, right));
                    }
                } else {
                    if !(left.is_terminal() && r_high.is_terminal()) {
                        stack.push((left, r_high));
                    }
                    if !(left.is_terminal() && r_low.is_terminal()) {
                        stack.push((left, r_low));
                    }
                }
            }
        }

        count
    }
}

pub mod node_cache {
    use std::num::NonZeroU64;
    use super::packed_bdd_node::PackedBddNode;
    use super::node_id::NodeId;
    use std::ops::{BitXor, Rem};
    use std::cmp::max;
    use super::ooo_apply::ApplyTask;

    pub struct NodeCache {
        capacity: NonZeroU64,
        index_after_last: usize,
        nodes: Vec<(PackedBddNode, NodeCacheSlot)>,
        table: Vec<NodeCacheSlot>,  // Hashtable pointing to the beginning of linked-lists in the nodes array.
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct NodeCacheSlot(u64);

    impl NodeCacheSlot {
        pub const UNDEFINED: NodeCacheSlot = NodeCacheSlot(u64::MAX);

        /// The conversion to a valid index. It can be safely done because we only support 64-bit machines.
        pub fn into_usize(self) -> usize {
            self.0 as usize
        }

        pub fn is_undefined(&self) -> bool {
            *self == Self::UNDEFINED
        }
    }

    impl From<u64> for NodeCacheSlot {
        fn from(value: u64) -> Self {
            NodeCacheSlot(value)
        }
    }

    impl From<usize> for NodeCacheSlot {
        fn from(value: usize) -> Self {
            NodeCacheSlot(value as u64)
        }
    }

    impl From<NodeCacheSlot> for u64 {
        fn from(value: NodeCacheSlot) -> Self {
            value.0
        }
    }

    /// This conversion is valid for cache slot ids that have a node inserted at that position.
    impl From<NodeCacheSlot> for NodeId {
        fn from(value: NodeCacheSlot) -> Self {
            NodeId::from(u64::from(value))
        }
    }

    impl NodeCache {
        const HASH_BLOCK: u64 = 1 << 13;
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

        pub fn new(table_capacity: usize, node_capacity: usize) -> NodeCache {
            debug_assert!(node_capacity > 2);
            debug_assert!(table_capacity > 0);
            NodeCache {
                capacity: NonZeroU64::new(table_capacity as u64).unwrap(),
                index_after_last: 2,    // Initially, there are two nodes already.
                table: vec![NodeCacheSlot::UNDEFINED; table_capacity],
                nodes: {
                    let mut result = Vec::with_capacity(node_capacity);
                    unsafe { result.set_len(node_capacity); }
                    result[0] = (PackedBddNode::ZERO, NodeCacheSlot::UNDEFINED);
                    result[1] = (PackedBddNode::ONE, NodeCacheSlot::UNDEFINED);
                    result
                }
            }
        }

        pub fn len(&self) -> usize {
            self.index_after_last
        }

        /// Try to add a node to the cache. If successful (or node exists), returns a `NodeId`.
        /// Otherwise, return a `NodeCacheSlot` that should be tried during next attempt.
        pub fn ensure(&mut self, node: &PackedBddNode) -> Result<NodeId, NodeCacheSlot> {
            let hash_slot = self.hash_position(&node);
            let linked_list_start = unsafe { self.table.get_unchecked_mut(hash_slot) };
            if linked_list_start.is_undefined() {
                // This hash has not been seen before. Create a new node for it.
                let fresh_slot = NodeCacheSlot::from(self.index_after_last);
                *linked_list_start = fresh_slot;
                self.index_after_last += 1;

                let slot_value = unsafe { self.nodes.get_unchecked_mut(fresh_slot.into_usize()) };
                *slot_value = (node.clone(), NodeCacheSlot::UNDEFINED);

                Ok(fresh_slot.into())
            } else {
                // There already is a value for this hash, try later.
                Err(*linked_list_start)
            }
        }

        /// Try to add a node to the cache at the given slot. The same as `ensure`, but we are not
        /// starting a new linked list, only continuing an existing one.
        pub fn ensure_at(&mut self, node: &PackedBddNode, slot: NodeCacheSlot) -> Result<NodeId, NodeCacheSlot> {
            let slot_value = unsafe { self.nodes.get_unchecked_mut(slot.into_usize()) };
            if &slot_value.0 == node {
                // This is a duplicate insertion, the node is already here.
                Ok(slot.into())
            } else if !slot_value.1.is_undefined() {
                // The node is not here, but there is another link in the chain that we can try.
                Err(slot_value.1)
            } else {
                // The chain ends here and we still haven't found the node. Create it.
                let fresh_slot = NodeCacheSlot::from(self.index_after_last);
                slot_value.1 = fresh_slot;
                self.index_after_last += 1;

                let slot_value = unsafe { self.nodes.get_unchecked_mut(fresh_slot.into_usize()) };
                *slot_value = (node.clone(), NodeCacheSlot::UNDEFINED);

                Ok(fresh_slot.into())
            }
        }

        pub fn ensure_at_2(&mut self, node: &ApplyTask, slot: NodeCacheSlot) -> Result<NodeId, NodeCacheSlot> {
            let slot_value = unsafe { self.nodes.get_unchecked_mut(slot.into_usize()) };
            if slot_value.0.eq_variable(node.variable) && slot_value.0.eq_low(node.results[1].as_node()) && slot_value.0.eq_high(node.results[0].as_node()) {
                // This is a duplicate insertion, the node is already here.
                Ok(slot.into())
            } else if !slot_value.1.is_undefined() {
                // The node is not here, but there is another link in the chain that we can try.
                Err(slot_value.1)
            } else {
                // The chain ends here and we still haven't found the node. Create it.
                let fresh_slot = NodeCacheSlot::from(self.index_after_last);
                slot_value.1 = fresh_slot;
                self.index_after_last += 1;

                let slot_value = unsafe { self.nodes.get_unchecked_mut(fresh_slot.into_usize()) };
                *slot_value = (PackedBddNode::pack(node.variable, node.results[1].as_node(), node.results[0].as_node()), NodeCacheSlot::UNDEFINED);

                Ok(fresh_slot.into())
            }
        }

        fn hash_position(&self, key: &PackedBddNode) -> usize {
            let low_link: u64 = key.get_low_link().into();
            let high_link: u64 = key.get_high_link().into();
            let low_hash = low_link.rotate_left(32).wrapping_mul(Self::SEED);
            let high_hash = high_link.wrapping_mul(Self::SEED);
            let block_index = low_hash.bitxor(high_hash).rem(Self::HASH_BLOCK);
            let base = max(low_link, high_link);
            (base + block_index).rem(self.capacity) as usize
        }

    }
}

pub mod task_cache {
    use std::ops::Rem;
    use super::node_id::NodeId;

    pub struct TaskCache {
        items: Vec<((NodeId, NodeId), NodeId)>
    }

    impl TaskCache {
        pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        const HASH_BLOCK: u64 = 1 << 13;

        pub fn new(capacity: usize) -> TaskCache {
            // The extra capacity ensures that we never have to modulo our hashes, since
            // hash block can be performed via bit-and, and the overall hash will be always ok.
            TaskCache {
                items: vec![((NodeId::ZERO, NodeId::ZERO), NodeId::ZERO); capacity + (Self::HASH_BLOCK as usize)]
            }
        }

        pub fn read(&self, task: (NodeId, NodeId)) -> (NodeId, usize) {
            let slot = self.hashed_index(task);
            let slot_value = unsafe { self.items.get_unchecked(slot ) };
            if slot_value.0 == task {
                (slot_value.1, slot)
            } else {
                (NodeId::UNDEFINED, slot)
            }
        }

        pub fn read_at(&self, task: (NodeId, NodeId), slot: usize) -> NodeId {
            let slot_value = unsafe { self.items.get_unchecked(slot ) };
            if slot_value.0 == task {
                slot_value.1
            } else {
                NodeId::UNDEFINED
            }
        }

        pub fn write(&mut self, task: (NodeId, NodeId), result: NodeId) {
            let slot = self.hashed_index(task);
            let slot_value = unsafe { self.items.get_unchecked_mut(slot ) };
            *slot_value = (task, result);
        }

        pub fn write_at(&mut self, slot: usize, task: (NodeId, NodeId), result: NodeId) {
            let slot_value = unsafe { self.items.get_unchecked_mut(slot ) };
            *slot_value = (task, result);
        }

        // Locality sensitive hashing algorithm, assuming that left nodes are a
        // mostly-growing sequence.
        fn hashed_index(&self, task: (NodeId, NodeId)) -> usize {
            let right_hash = u64::from(task.1).wrapping_mul(Self::SEED);
            let block_index = right_hash.rem(Self::HASH_BLOCK);
            let block_start: u64 = u64::from(task.0);

            unsafe {
                // Usually not that important, but seems to be actually helping for large BDDs.
                let pointer: *const ((NodeId, NodeId), NodeId) =
                    self.items.get_unchecked((block_start as usize) + 128);
                std::arch::x86_64::_mm_prefetch::<1>(pointer as *const i8);
            }
            (block_start + block_index) as usize
        }

    }
}

pub mod apply {
    use super::bdd::Bdd;
    use super::node_id::NodeId;
    use super::packed_bdd_node::PackedBddNode;
    use super::bdd_dfs::UnsafeStack;
    use super::variable_id::VariableId;
    use super::node_cache::NodeCache;
    use super::task_cache::TaskCache;
    use std::result::Result::Err;

    #[derive(Copy, Clone, Eq, PartialEq)]
    struct ApplyTask {
        offset: u8,
        variable: VariableId,
        task: (NodeId, NodeId),
        results: [NodeId; 2],
        task_cache_slot: usize,
    }

    impl ApplyTask {

        pub fn new(offset: u8, task: (NodeId, NodeId)) -> ApplyTask {
            ApplyTask {
                offset: offset << 1,
                task,
                variable: VariableId::UNDEFINED,
                results: [NodeId::UNDEFINED, NodeId::UNDEFINED],
                task_cache_slot: 0,
            }
        }
    }

    pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> (usize, usize) {
        let height_limit = left_bdd.get_height() + right_bdd.get_height();
        let mut task_cache = TaskCache::new(left_bdd.node_count());
        let mut node_cache = NodeCache::new(left_bdd.node_count(), 2 * left_bdd.node_count());
        let mut task_count = 0;

        let mut stack = UnsafeStack::new(height_limit);
        stack.push(ApplyTask::new(0, (left_bdd.get_root_id(), right_bdd.get_root_id())));

        while !stack.is_empty() {
            let top = stack.peek();

            let offset = (top.offset >> 1) as usize;    // Must be here otherwise top's lifetime will not end before we want to push.
            let mut result = NodeId::UNDEFINED;
            if top.offset & 1 == 0 {
                top.offset |= 1;   // mark task as expanded

                let (left, right) = top.task;
                if left.is_one() || right.is_one() {            // Certain one
                    result = NodeId::ONE;
                } else if left.is_zero() && right.is_zero() {   // Certain zero
                    result = NodeId::ZERO;
                } else {
                    let (cached, slot) = task_cache.read(top.task);
                    if !cached.is_undefined() {
                        result = cached;
                    } else {
                        top.task_cache_slot = slot;
                        // Actually expand.
                        task_count += 1;

                        let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                        let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                        let (l_var, l_low, l_high) = left_node.unpack();
                        let (r_var, r_low, r_high) = right_node.unpack();

                        // This explicit "switch" is slightly faster. Not sure exactly why, but
                        // it is probably easier to branch predict.
                        if l_var == r_var {
                            top.variable = l_var;
                            stack.push(ApplyTask::new(1, (l_high, r_high)));
                            stack.push(ApplyTask::new(2, (l_low, r_low)));
                        } else if l_var < r_var {
                            top.variable = l_var;
                            stack.push(ApplyTask::new(1, (l_high, right)));
                            stack.push(ApplyTask::new(2, (l_low, right)));
                        } else {
                            top.variable = r_var;
                            stack.push(ApplyTask::new(1, (left, r_high)));
                            stack.push(ApplyTask::new(2, (left, r_low)));
                        }
                    }
                }
            } else {
                // Task is decoded, we have to create a new node for it.
                let (result_low, result_high) = (top.results[1], top.results[0]);
                if result_low == result_high {
                    task_cache.write_at(top.task_cache_slot, top.task, result_low);
                    result = result_low;
                } else {
                    let node = PackedBddNode::pack(top.variable, result_low, result_high);

                    let mut cached = node_cache.ensure(&node);
                    while let Err(slot) = cached {
                        cached = node_cache.ensure_at(&node, slot);
                    }
                    result = cached.unwrap();
                    task_cache.write_at(top.task_cache_slot, top.task, result);
                }
            }

            if !result.is_undefined() {
                stack.pop();
                if !stack.is_empty() {
                    let parent = stack.peek_at(offset);
                    // high = 1, low = 2, so they will be saved in reverse order.
                    let slot = unsafe { parent.results.get_unchecked_mut(offset - 1) };
                    *slot = result;
                }
            }
        }

        (node_cache.len(), task_count)
    }

}

pub mod ooo_apply {
    use super::packed_bdd_node::PackedBddNode;
    use super::node_cache::{NodeCache, NodeCacheSlot};
    use super::bdd::Bdd;
    use super::node_id::NodeId;
    use super::variable_id::VariableId;
    use super::bdd_dfs::UnsafeStack;
    use std::ops::Rem;

    const ROB_MASK: u64 = 1 << 63;

    // Rob slot is stored as is, node id has the highest bit set to 1. Intuition is
    // that rob slot can be checked repeatedly, while result is read only once.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct NodeIdOrRobSlot(u64);

    impl NodeIdOrRobSlot {
        pub const UNDEFINED: NodeIdOrRobSlot = NodeIdOrRobSlot(u64::MAX);

        pub fn is_rob(&self) -> bool {
            self.0 & ROB_MASK != 0
        }

        pub fn is_undefined(&self) -> bool {
            *self == Self::UNDEFINED
        }

        pub fn as_rob(self) -> RobSlot {
            RobSlot::from(self.0 as u32)    // Will erase the highest bit.
        }

        pub fn as_node(self) -> NodeId {
            NodeId::from(self.0)
        }

    }

    impl From<NodeId> for NodeIdOrRobSlot {
        fn from(value: NodeId) -> Self {
            NodeIdOrRobSlot(u64::from(value))
        }
    }

    impl From<RobSlot> for NodeIdOrRobSlot {
        fn from(value: RobSlot) -> Self {
            NodeIdOrRobSlot(u64::from(u32::from(value)) | ROB_MASK)
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct ApplyTask {
        offset: u8,
        pub variable: VariableId,
        task: (NodeId, NodeId),
        pub results: [NodeIdOrRobSlot; 2],
        task_cache_slot: usize,
    }

    impl ApplyTask {

        pub fn new(offset: u8, task: (NodeId, NodeId)) -> ApplyTask {
            ApplyTask {
                offset: offset << 1,
                task,
                variable: VariableId::UNDEFINED,
                results: [NodeIdOrRobSlot::UNDEFINED, NodeIdOrRobSlot::UNDEFINED],
                task_cache_slot: usize::MAX,
            }
        }
    }

    pub struct TaskCache {
        items: Vec<((NodeId, NodeId), NodeIdOrRobSlot)>
    }

    impl TaskCache {
        pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        const HASH_BLOCK: u64 = 1 << 13;

        pub fn new(capacity: usize) -> TaskCache {
            // The extra capacity ensures that we never have to modulo our hashes, since
            // hash block can be performed via bit-and, and the overall hash will be always ok.
            TaskCache {
                items: vec![((NodeId::ZERO, NodeId::ZERO), NodeIdOrRobSlot::from(NodeId::ZERO)); capacity + (Self::HASH_BLOCK as usize)]
            }
        }

        pub fn read(&self, task: (NodeId, NodeId)) -> (NodeIdOrRobSlot, usize) {
            let slot = self.hashed_index(task);
            let slot_value = unsafe { self.items.get_unchecked(slot ) };
            if slot_value.0 == task {
                (slot_value.1, slot)
            } else {
                (NodeIdOrRobSlot::UNDEFINED, slot)
            }
        }

        pub fn read_at(&self, task: (NodeId, NodeId), slot: usize) -> NodeIdOrRobSlot {
            let slot_value = unsafe { self.items.get_unchecked(slot ) };
            if slot_value.0 == task {
                slot_value.1
            } else {
                NodeIdOrRobSlot::UNDEFINED
            }
        }

        pub fn write(&mut self, task: (NodeId, NodeId), result: NodeIdOrRobSlot) {
            let slot = self.hashed_index(task);
            let slot_value = unsafe { self.items.get_unchecked_mut(slot ) };
            *slot_value = (task, result);
        }

        pub fn write_at(&mut self, slot: usize, task: (NodeId, NodeId), result: NodeIdOrRobSlot) {
            let slot_value = unsafe { self.items.get_unchecked_mut(slot ) };
            *slot_value = (task, result);
        }

        // Locality sensitive hashing algorithm, assuming that left nodes are a
        // mostly-growing sequence.
        fn hashed_index(&self, task: (NodeId, NodeId)) -> usize {
            let right_hash = u64::from(task.1).wrapping_mul(Self::SEED);
            let block_index = right_hash.rem(Self::HASH_BLOCK);
            let block_start: u64 = u64::from(task.0);

            unsafe {
                // Usually not that important, but seems to be actually helping for large BDDs.
                let pointer: *const ((NodeId, NodeId), NodeIdOrRobSlot) =
                    self.items.get_unchecked((block_start as usize) + 128);
                std::arch::x86_64::_mm_prefetch::<1>(pointer as *const i8);
            }
            (block_start + block_index) as usize
        }

    }

    pub struct ReorderBuffer {
        buffer: Vec<(u64, u16)>,
        next_free: RobSlot,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct RobSlot(u32);

    impl RobSlot {
        pub const UNDEFINED: RobSlot = RobSlot(u32::MAX);

        pub fn into_usize(self) -> usize {
            self.0 as usize
        }

        pub fn is_undefined(&self) -> bool {
            *self == Self::UNDEFINED
        }
    }

    impl From<u32> for RobSlot {
        fn from(value: u32) -> Self {
            RobSlot(value)
        }
    }

    impl From<RobSlot> for u32 {
        fn from(value: RobSlot) -> Self {
            value.0
        }
    }

    impl ReorderBuffer {

        pub fn new(capacity: usize) -> ReorderBuffer {
            // Create a linked list starting in zero and going through all slots in the vector.
            let mut list = vec![(0, 0); capacity];
            for i in 0..list.len() {
                list[i] = ((i + 1) as u64, 0);
            }
            // Last element has no successor, so we set it to u64::MAX.
            let last_index = list.len() - 1;
            list[last_index] = (u64::MAX, 0);
            ReorderBuffer {
                buffer: list,
                next_free: RobSlot(0)
            }
        }

        pub fn is_full(&self) -> bool {
            self.next_free == RobSlot::UNDEFINED
        }

        pub fn allocate_and_ref_slot(&mut self) -> RobSlot {
            debug_assert!(!self.is_full());
            let slot_id = self.next_free;
            let slot_value = unsafe { self.buffer.get_unchecked_mut(slot_id.into_usize()) };

            // Free slots are a linked list, hence slot value is either next free slot or undefined.
            self.next_free = RobSlot::from(slot_value.0 as u32);
            // Erase the linked list pointer, meaning that this slot contains an unfinished task.
            *slot_value = (u64::MAX, 1);
            // Return a pointer to the newly allocated ROB slot.
            slot_id
        }

        pub fn ref_slot(&mut self, slot: RobSlot) {
            let slot_id: u32 = slot.into();
            let slot_value = unsafe { self.buffer.get_unchecked_mut(slot_id as usize) };
            slot_value.1 += 1;
        }

        pub fn deref_slot(&mut self, slot: RobSlot) {
            let slot_id: u32 = slot.into();
            debug_assert!((slot_id as usize) < self.buffer.len()); // Check bounds.
            let slot_value = unsafe { self.buffer.get_unchecked_mut(slot_id as usize) };

            // Erase slot value and replace with pointer of next free slot.
            slot_value.1 -= 1;
            if slot_value.1 == 0 {
                slot_value.0 = u64::from(u32::from(self.next_free));
                // Update next free value such that it points to this newly freed slot.
                self.next_free = slot;
            }
        }

        pub fn get_slot_value(&self, slot: RobSlot) -> NodeId {
            NodeId::from(unsafe { self.buffer.get_unchecked(slot.0 as usize).0 })
        }

        pub fn set_slot_value(&mut self, slot: RobSlot, id: NodeId) {
            let slot_value = unsafe { self.buffer.get_unchecked_mut(slot.0 as usize) };
            slot_value.0 = id.into();
        }

    }


    pub struct ExecutionRetireQueue<const LEN: usize> {
        queue: Vec<(ApplyTask, NodeCacheSlot, RobSlot)>,
        retire_head: usize,
        execution_head: usize,
        execution_tail: usize,
        next_retire_head: usize,
        next_execution_head: usize,
        next_execution_tail: usize,
    }

    impl<const LEN: usize> ExecutionRetireQueue<LEN> {

        pub fn new() -> ExecutionRetireQueue<LEN> {
            let mut queue = Vec::with_capacity(LEN);
            unsafe { queue.set_len(LEN); }
            ExecutionRetireQueue {
                queue,
                retire_head: 0,
                execution_head: 0,
                execution_tail: 0,
                next_retire_head: 0,
                next_execution_head: 0,
                next_execution_tail: 0,
            }
        }

        /// Checks whether this execution-retire queue has free slots into which new tasks
        /// can be enqueued.
        pub fn is_full(&self) -> bool {
            (self.execution_tail + 1) % LEN == self.retire_head
        }

        pub fn is_empty(&self) -> bool {
            self.execution_tail == self.execution_head && self.execution_head == self.retire_head
        }

        /// Return true if the queue contains at least one task in the execution queue.
        pub fn can_execute(&self) -> bool {
            self.execution_head != self.execution_tail
        }

        /// Return true if the queue contains at least one task in the retire queue.
        pub fn can_retire(&self) -> bool {
            self.retire_head != self.execution_head
        }

        /// Add a new task into this queue, that will be marked for execution.
        ///
        /// **Safety:** The method can be only called on a queue that is not full!
        pub fn enqueue_for_execution(&mut self, rob: RobSlot, task: &ApplyTask) {
            debug_assert!(!self.is_full());
            let slot = unsafe { self.queue.get_unchecked_mut(self.execution_tail) };
            *slot = (task.clone(), NodeCacheSlot::UNDEFINED, rob);
            self.next_execution_tail = (self.execution_tail + 1) % LEN
        }

        /// Obtain the reference to the task that should be executed next.
        ///
        /// **Safety:** If the method is called on an empty queue, the resulting reference is valid,
        /// but its contents are undefined.
        pub fn execute_task_reference(&mut self) -> &mut (ApplyTask, NodeCacheSlot, RobSlot) {
            debug_assert!(self.can_execute());
            unsafe { self.queue.get_unchecked_mut(self.execution_head) }
        }

        /// Move the head of the execution queue into the retire queue.
        ///
        /// **Safety:** The method is only valid when the execution queue is not empty. Additionally,
        /// you should only call this once both result slots and a task cache slot of the pending
        /// task have been filled.
        pub fn move_to_retire(&mut self) {
            debug_assert!(self.can_execute());
            self.next_execution_head = (self.execution_head + 1) % LEN;
        }

        /// Obtain the reference to the task that should be retired next.
        ///
        /// **Safety:** If the method is called on an empty retire queue, the result is a valid
        /// reference, but its contents are undefined.
        pub fn retire_task_reference(&mut self) -> &mut (ApplyTask, NodeCacheSlot, RobSlot) {
            debug_assert!(self.can_retire());
            unsafe { self.queue.get_unchecked_mut(self.retire_head) }
        }

        /// Free up the head of the retirement queue.
        ///
        /// **Safety:** The operation is valid only if the retire queue is not empty. Additionally,
        /// retiring a task before it is committed to node storage, task cache and ROB will break
        /// subsequent invariants.
        pub fn retire(&mut self) {
            debug_assert!(self.can_retire());
            self.next_retire_head = (self.retire_head + 1) % LEN;
        }

        pub fn commit(&mut self) {
            self.execution_head = self.next_execution_head;
            self.retire_head = self.next_retire_head;
            self.execution_tail = self.next_execution_tail;
        }

    }

    pub fn ooo_apply(left_bdd: &Bdd, right_bdd: &Bdd) -> (usize, usize) {
        let height_limit = left_bdd.get_height() + right_bdd.get_height();
        let mut task_cache = TaskCache::new(left_bdd.node_count());
        let mut stack = UnsafeStack::new(height_limit);
        let mut rob = ReorderBuffer::new(height_limit);
        let mut queue = ExecutionRetireQueue::<64>::new();
        let mut node_cache = NodeCache::new(left_bdd.node_count(), 2 * left_bdd.node_count());
        let mut task_count = 0;

        stack.push(ApplyTask::new(0, (left_bdd.get_root_id(), right_bdd.get_root_id())));

        while !stack.is_empty() {
            //println!("iter");
            let top = stack.peek();

            let offset = (top.offset >> 1) as usize;    // Must be here otherwise top's lifetime will not end before we want to push.
            let mut result = NodeIdOrRobSlot::UNDEFINED;
            if top.offset & 1 == 0 {
                top.offset |= 1;   // mark task as expanded

                let (left, right) = top.task;
                if left.is_one() || right.is_one() {            // Certain one
                    result = NodeId::ONE.into();
                } else if left.is_zero() && right.is_zero() {   // Certain zero
                    result = NodeId::ZERO.into();
                } else {
                    let (cached, slot) = task_cache.read(top.task);
                    if !cached.is_undefined() {
                        if cached.is_rob() {
                            rob.ref_slot(cached.as_rob());
                        }
                        result = cached.into();
                    } else {
                        top.task_cache_slot = slot;
                        // Actually expand.
                        task_count += 1;

                        let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                        let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                        let (l_var, l_low, l_high) = left_node.unpack();
                        let (r_var, r_low, r_high) = right_node.unpack();

                        // This explicit "switch" is slightly faster. Not sure exactly why, but
                        // it is probably easier to branch predict.
                        if l_var == r_var {
                            top.variable = l_var;
                            stack.push(ApplyTask::new(1, (l_high, r_high)));
                            stack.push(ApplyTask::new(2, (l_low, r_low)));
                        } else if l_var < r_var {
                            top.variable = l_var;
                            stack.push(ApplyTask::new(1, (l_high, right)));
                            stack.push(ApplyTask::new(2, (l_low, right)));
                        } else {
                            top.variable = r_var;
                            stack.push(ApplyTask::new(1, (left, r_high)));
                            stack.push(ApplyTask::new(2, (left, r_low)));
                        }
                    }
                }
            } else if !queue.is_full() {
                // TODO: Prove that ROB cannot be full at this point.
                let rob_slot = rob.allocate_and_ref_slot();
                result = rob_slot.into();
                queue.enqueue_for_execution(rob_slot, top);
                task_cache.write_at(top.task_cache_slot, top.task, rob_slot.into());
            }

            if !result.is_undefined() {
                stack.pop();
                if !stack.is_empty() {
                    let parent = stack.peek_at(offset);
                    // high = 1, low = 2, so they will be saved in reverse order.
                    let slot = unsafe { parent.results.get_unchecked_mut(offset - 1) };
                    *slot = result;
                }
            }

            if queue.can_execute() {
                let (task, node_cache_slot, dest) = queue.execute_task_reference();
                let mut result_high = task.results[0];
                let mut result_low = task.results[1];

                if result_low.is_rob() {
                    let slot = result_low.as_rob();
                    let result = rob.get_slot_value(slot);
                    if !result.is_undefined() {
                        rob.deref_slot(slot);
                        result_low = result.into();
                        task.results[1] = result.into();
                    }
                } else if result_high.is_rob() {
                    // For some reason, this statistically happens much less often,
                    // so we don't check it explicitly, but only if we really have to.
                    let slot = result_high.as_rob();
                    let result = rob.get_slot_value(slot);
                    if !result.is_undefined() {
                        rob.deref_slot(slot);
                        result_high = result.into();
                        task.results[0] = result.into();
                    }
                }

                if !result_low.is_rob() && !result_high.is_rob() {
                    let result_high = result_high.as_node();
                    let result_low = result_low.as_node();

                    if result_low == result_high {
                        rob.set_slot_value(*dest, result_low);
                        *dest = RobSlot::UNDEFINED; // Mark the task as retired.
                        task_cache.write_at(task.task_cache_slot, task.task, result_low.into());
                    } else {
                        match node_cache.ensure(&PackedBddNode::pack(task.variable, result_low, result_high)) {
                            Ok(id) => {
                                // Node is already cached, just update result.
                                rob.set_slot_value(*dest, id);
                                *dest = RobSlot::UNDEFINED;
                                task_cache.write_at(task.task_cache_slot, task.task, id.into());
                            }
                            Err(slot) => {
                                *node_cache_slot = slot;
                            }
                        }
                    }
                    queue.move_to_retire();
                }
            }

            if queue.can_retire() {
                let (task, node_cache_slot, dest) = queue.retire_task_reference();
                if dest.is_undefined() { // The task was retired during the execute step.
                    queue.retire()
                } else {
                    match node_cache.ensure_at_2(task, *node_cache_slot) {
                        Ok(id) => {
                            rob.set_slot_value(*dest, id);
                            task_cache.write_at(task.task_cache_slot, task.task, id.into());
                            queue.retire();
                        }
                        Err(slot) => {
                            *node_cache_slot = slot;
                        }
                    }
                }
            }

            queue.commit();

        }

        (node_cache.len(), task_count)
    }

}