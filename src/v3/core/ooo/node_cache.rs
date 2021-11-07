use std::num::NonZeroU64;
use crate::v3::core::packed_bdd_node::PackedBddNode;
use std::ops::{BitXor, Rem};
use std::cmp::max;
use crate::v3::core::node_id::NodeId;

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
    const HASH_BLOCK: u64 = 1 << 14;
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

    pub fn check_capacity(&mut self)  {
        if self.index_after_last == self.nodes.len() {
            unimplemented!("Re-allocate nodes.")
        }
    }

    fn hash_position(&self, key: &PackedBddNode) -> usize {
        let low_link: u64 = key.get_low_link().into();
        let high_link: u64 = key.get_high_link().into();
        let low_hash = low_link.wrapping_mul(Self::SEED);
        let high_hash = high_link.wrapping_mul(Self::SEED);
        let block_index = low_hash.bitxor(high_hash).rem(Self::HASH_BLOCK);
        let base = max(low_link, high_link);
        (base + block_index).rem(self.capacity) as usize
    }

    pub fn export_nodes(self) -> Vec<PackedBddNode> {
        self.nodes.into_iter().take(self.index_after_last).map(|(node, _)| { /*println!("{:?}", node); */node }).collect()
    }

}

#[cfg(test)]
mod test {
    use crate::v3::core::ooo::node_cache::{NodeCache, NodeCacheSlot};
    use crate::v3::core::packed_bdd_node::PackedBddNode;
    use crate::v3::core::node_id::NodeId;

    #[test]
    pub fn basic_node_cache_test() {
        let mut cache = NodeCache::new(2, 16);
        let node_1 = PackedBddNode::pack(123u32.into(), 14u64.into(), 12u64.into());
        let node_2 = PackedBddNode::pack(456u32.into(), 14u64.into(), 12u64.into());

        // Add a first node.
        let result = cache.ensure(&node_1).unwrap();
        assert_eq!(NodeId::from(2u64), result);

        // Check that the node is really there.
        let result = cache.ensure(&node_1).unwrap_err();
        assert_eq!(NodeCacheSlot::from(2u64), result);
        let result = cache.ensure_at(&node_1, result).unwrap();
        assert_eq!(NodeId::from(2u64), result);

        // The second node will collide with the first because it only differs in the variable.
        let result = cache.ensure(&node_2).unwrap_err();
        assert_eq!(NodeCacheSlot::from(2u64), result);
        let result = cache.ensure_at(&node_2, result).unwrap();
        assert_eq!(NodeId::from(3u64), result);

        // But the first one is still there.
        let result = cache.ensure(&node_1).unwrap_err();
        assert_eq!(NodeCacheSlot::from(2u64), result);
        let result = cache.ensure_at(&node_1, result).unwrap();
        assert_eq!(NodeId::from(2u64), result);

        // And the second one as well.
        let result = cache.ensure(&node_2).unwrap_err();
        assert_eq!(NodeCacheSlot::from(2u64), result);
        let result = cache.ensure_at(&node_2, result).unwrap_err();
        assert_eq!(NodeCacheSlot::from(3u64), result);
        let result = cache.ensure_at(&node_2, result).unwrap();
        assert_eq!(NodeId::from(3u64), result);
    }

}