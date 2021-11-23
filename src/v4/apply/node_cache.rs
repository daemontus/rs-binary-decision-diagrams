use std::cmp::max;
use crate::{FromIndex, IntoIndex};
use super::super::core::{Node, NodeIndex};

/// Node cache serves as a temporary storage for BDD nodes which is responsible for ensuring that
/// each node is given a unique index, and that there are no duplicate nodes.
///
/// Compared to `TaskCache`, `NodeCache` cannot be leaky and has to always represent all stored
/// nodes faithfully (if it weren't the case, the errors would quickly propagate throughout the
/// algorithm).
///
/// However, this does not mean that we can't make the hashed indices at least partially local.
/// In this case, the principle is very simple: The table resolves collisions by growing a linked
/// list of nodes which is tracked in the main `nodes` vector. However, to obtain the first entry
/// in the linked list, one has to go through the "hashed" `table`. For this table, the hash is
/// simply the maximum of the two low and high links of the node that we are storing.
///
/// This may sound like it is going to produce a large number of collisions, but in fact it is
/// quite reasonable, because for most BDD nodes, the number of incoming edges is quite small.
/// This number then directly influences how many opportunities the node has to be the cause of
/// a collision. Additionally, this number will be again a mostly growing sequence, because
/// newly created nodes have a statistical tendency to reference other recently created nodes.
/// Furthermore, a new node can only reference nodes that are already created, we know that this
/// number will never outgrow the current table size and no bounds check or modulo computation
/// is necessary.
///
/// To grow the cache, we simply double the size of both tables. Interestingly, since the "hash"
/// remains the same, we don't need to do any rehashing. However, this also means that all
/// collisions are deterministic and will appear in the updated table as well.
pub struct NodeCache {
    index_after_last: u64,
    nodes: Vec<(Node, NodeCacheSlot)>,
    table: Vec<NodeCacheSlot>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NodeCacheSlot(u64);

impl NodeCacheSlot {
    pub const UNDEFINED: NodeCacheSlot = NodeCacheSlot(u64::MAX);

    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

    /// This conversion is valid for cache slots that already have a node inserted at that position.
    pub unsafe fn into_node(self) -> NodeIndex {
        self.0.into()
    }
}

impl From<u64> for NodeCacheSlot {
    fn from(value: u64) -> Self {
        NodeCacheSlot(value)
    }
}

impl From<NodeCacheSlot> for u64 {
    fn from(value: NodeCacheSlot) -> Self {
        value.0
    }
}

impl FromIndex for NodeCacheSlot {
    fn from_index(index: usize) -> Self {
        NodeCacheSlot(u64::from_index(index))
    }
}

impl IntoIndex for NodeCacheSlot {
    fn into_index(self) -> usize {
        self.0.into_index()
    }
}

impl NodeCache {

    /// Create a new node cache with the given initial capacity. To make the resulting node indices
    /// compatible with our BDD conventions, the cache will be also pre-populated with two terminal
    /// nodes at their assumed positions. As such, the initial capacity must be able to accommodate
    /// at least these two nodes.
    pub fn new(initial_capacity: u64) -> NodeCache {
        assert!(initial_capacity >= 2);
        let initial_capacity = initial_capacity.into_index();
        NodeCache {
            index_after_last: 2,    // Initially, there are two nodes inserted.
            table: vec![NodeCacheSlot::UNDEFINED; initial_capacity],
            nodes: {
                // Create a block of uninitialized memory.
                let mut result = Vec::with_capacity(initial_capacity);
                unsafe { result.set_len(initial_capacity); }
                // And fill the first two slots.
                result[0] = (Node::ZERO, NodeCacheSlot::UNDEFINED);
                result[1] = (Node::ONE, NodeCacheSlot::UNDEFINED);
                result
            }
        }
    }

    pub fn len(&self) -> usize {
        self.index_after_last.into_index()
    }

    /// Try to add a node into the cache. If successful (or the node already exists), returns
    /// a `NodeIndex`. Otherwise, return a `NodeCacheSlot` that should be tried during
    /// the next attempt.
    ///
    /// The function is unsafe because it assumes the cache has sufficient capacity to insert
    /// a node.
    pub fn ensure(&mut self, node: &Node) -> Result<NodeIndex, NodeCacheSlot> {
        let hash_slot = self.hash_position(&node);
        let linked_list_start = unsafe {
            self.table.get_unchecked_mut(hash_slot)
        };
        if linked_list_start.is_undefined() {
            // This hash has not been seen before. Create a new node for it.
            let fresh_slot = NodeCacheSlot::from(self.index_after_last);
            *linked_list_start = fresh_slot;
            self.index_after_last += 1;

            let slot_value = unsafe {
                self.nodes.get_unchecked_mut(fresh_slot.into_index())
            };
            *slot_value = (node.clone(), NodeCacheSlot::UNDEFINED);

            Ok(unsafe { fresh_slot.into_node() })
        } else {
            // There already is a value for this hash, try later.
            Err(*linked_list_start)
        }
    }

    /// Try to add a node to the cache at the given slot. The same as `ensure`, but we are not
    /// starting a new linked list, only continuing an existing one.
    ///
    /// The function is unsafe because it assumes the cache has sufficient capacity to insert
    /// a node.
    pub fn ensure_at(&mut self, node: &Node, slot: NodeCacheSlot) -> Result<NodeIndex, NodeCacheSlot> {
        let slot_value = unsafe { self.nodes.get_unchecked_mut(slot.into_index()) };
        if &slot_value.0 == node {
            // This is a duplicate insertion, the node is already here.
            Ok(unsafe { slot.into_node() })
        } else if !slot_value.1.is_undefined() {
            // The node is not here, but there is another link in the chain that we can try.
            Err(slot_value.1)
        } else {
            // The chain ends here and we still haven't found the node. Create it.
            let fresh_slot = NodeCacheSlot::from(self.index_after_last);
            slot_value.1 = fresh_slot;
            self.index_after_last += 1;

            let slot_value = unsafe { self.nodes.get_unchecked_mut(fresh_slot.into_index()) };
            *slot_value = (node.clone(), NodeCacheSlot::UNDEFINED);

            Ok(unsafe { fresh_slot.into_node() })
        }
    }

    fn hash_position(&self, key: &Node) -> usize {
        let low_link = key.get_low_link().into_index();
        let high_link = key.get_high_link().into_index();
        max(low_link, high_link)
    }
/*
    /// Ensures that the cache can accommodate at least `minimal_capacity` additional nodes.
    /// The returned number is the actual number of nodes that can be inserted without issues.
    pub fn ensure_capacity(&mut self, minimal_capacity: u64) -> u64 {
        let free_slots = u64::from_index(self.nodes.len()) - self.index_after_last;
        if free_slots >= minimal_capacity {
            return free_slots;
        }

        self.nodes.reserve_exact(self.nodes.len());
        self.table.reserve_exact(self.table.len());
        unsafe {
            self.nodes.set_len(self.nodes.capacity());
            self.table.set_len(self.table.capacity());
        }

        return u64::from_index(self.nodes.len()) - self.index_after_last;
    }
*/
}