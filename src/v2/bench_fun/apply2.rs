use crate::v2::bench_fun::deps::{BddNode, NodeId, PackedBddNode};
use bitintr::Pdep;
use likely_stable::unlikely;
use std::cmp::{max, min};
use std::mem::swap;
use std::num::NonZeroU64;
use std::ops::{BitAnd, BitOr, BitXor, Rem};
/*
/// Every stack item has 5 64-bit values. These can represent different things:
///  1) (NodeId, NodeId, _, _, _) is a task waiting to be decoded.
///  2) (NodeId, u64::MAX, _, _, _) is a completed task with NodeId as its result.
///  3) (NodeId, NodeId, VariableId, TaskSlot, _) is a decoded task (TaskSlot is a slot in the
///     task cache).
///  4) (PackedBddNode, TaskSlot, NodeSlot, NextRetire) is a task waiting for retirement.
///
/// A task starts as (1). If it is terminal or saved in cache, it is immediately resolved
/// and becomes (2). Otherwise it becomes (3) and spawns two new tasks. Once these tasks
/// are completed (i.e. (2)), it can move either to (2) (if no node needs to be created),
/// or (4). Once in (4), the task can either continue as (4) (if NodeSlot is not right),
/// or move to (2) when the right NodeSlot is found.
///
/// To distinguish between (3) and (4), we use the fact that in (3), the node IDs are at most
/// 48 bits, while in (4), the high link will always have a variable encoded in it as well.
///
pub struct Stack2 {
    push_at: usize,
    retire_at: usize,
    items: Vec<(u64, u64, u64, u64, u64)>,
}

impl Stack2 {
    const NEEDS_DECODE_MASK: u64 = (u16::MAX as u64) << 48;

    pub fn new(variable_count: u16) -> Stack2 {
        let mut stack = Stack2 {
            push_at: 1,
            retire_at: 0,
            items: vec![(0, 0, 0, 0, 0); 3 * usize::from(variable_count) + 2],
        };
        // Add a fake "result" entry at the bottom of the stack.
        stack.items[0] = (0, u64::MAX, 0, 0, 0);
        stack
    }

    /// This is actually an emptiness check, because if there is only one last
    /// entry, it is necessarily the result of the "root" task.
    #[inline]
    pub fn has_last_entry(&self) -> bool {
        self.push_at == 2
    }

    /// Add a new task to the top of the stack.
    #[inline]
    pub fn push(&mut self, task: (NodeId, NodeId)) {
        let slot = unsafe { self.items.get_unchecked_mut(self.push_at) };
        *slot = (u64::from(task.0), u64::from(task.1), 0, 0, 0);
        self.push_at += 1;
    }

    /// Finish the top task on stack. If there is a task waiting for decoding underneath,
    /// swap the tasks.
    #[inline]
    pub fn finish(&mut self, result: NodeId) {
        let before_top_index = self.push_at - 2;
        let top_index = self.push_at - 1;
        let before_top = unsafe { self.items.get_unchecked_mut(before_top_index) };

        let before_top_decoded = (before_top.1 & Self::NEEDS_DECODE_MASK) != 0;
        if before_top_decoded {
            // entry[-2] is also decoded, just place the result on top.
            unsafe {
                let slot = self.items.get_unchecked_mut(top_index);
                slot.0 = u64::from(result);
                slot.1 = u64::MAX;
            }
        } else {
            // entry[-2] can be decoded - we need to put it on top
            let new_top = (before_top.0, before_top.1);
            before_top.0 = u64::from(result);
            todo!()
        }
    }
}

/// Again, we rely on the fact that zero/one are reserved as terminal and should never
/// be explicitly inserted.
pub struct NodeCache2 {
    block_mask: u64,
    next_node_id: u64,
    min_bits: u32,
    capacity: NonZeroU64,
    nodes: Vec<(PackedBddNode, u64)>,
    hashes: Vec<u64>,
}

impl NodeCache2 {
    pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(initial_capacity: usize) -> NodeCache2 {
        NodeCache2 {
            block_mask: (1 << 14) - 1,
            next_node_id: 2, // Skip first two nodes
            min_bits: 0,
            capacity: NonZeroU64::new(initial_capacity as u64).unwrap(),
            nodes: vec![(PackedBddNode(0, 0), 0); initial_capacity + initial_capacity / 2],
            hashes: vec![0; initial_capacity],
        }
    }

    /// Get the next possible slot for node placement.
    #[inline]
    pub unsafe fn next_slot(&mut self, slot: NodeId) -> NodeId {
        let entry = unsafe { self.nodes.get_unchecked_mut(slot.as_index_unchecked()) };
        if entry.1 != 0 {
            // Continue this chain.
            NodeId(entry.1)
        } else {
            // Chain ended. Allocate new node.
            let new_id = self.next_node_id;
            self.next_node_id += 1;
            entry.1 = new_id;
            debug_assert!(self.next_node_id < (self.nodes.len() as u64));
            NodeId(new_id)
        }
    }

    /// Try to read an id of a node from a specific slot. If the slot is empty,
    /// a new entry is allocated at that position. If the slot does not
    /// contain this node but some other data, returns `NodeId::UNDEFINED`.
    #[inline]
    pub unsafe fn ensure_slot(&mut self, slot: NodeId, node: PackedBddNode) -> NodeId {
        let entry = unsafe { self.nodes.get_unchecked_mut(slot.as_index_unchecked()) };
        if entry.0 == node {
            // The node is found.
            slot
        } else if (entry.0 .0 | entry.0 .1) == 0 {
            // Empty slot is found.
            entry.0 = node;
            slot
        } else {
            NodeId::UNDEFINED
        }
    }

    /// Find first *possible* position to place the given node at.
    #[inline]
    pub fn find_slot(&mut self, node: BddNode) -> NodeId {
        let low = u64::from(node.low_link());
        let high = u64::from(node.high_link());

        let block_offset = {
            let low_hash = low.wrapping_mul(Self::SEED);
            let high_hash = high.wrapping_mul(Self::SEED);
            low_hash.bitxor(high_hash).bitand(self.block_mask)
        };

        let block_position = {
            let max = max(low, high);
            let min = min(low, high);

            let min_bits = self.min_bits;
            let max = max.wrapping_shl(min_bits);
            let min = min & ((1 << min_bits) - 1);
            max.bitor(min)
        };

        let table_index = (block_position + block_offset).rem(self.capacity);

        let slot = unsafe { self.hashes.get_unchecked_mut(table_index as usize) };

        if *slot != 0 {
            // We have seen this hash before, so let's return the first node in that chain.
            NodeId(*slot)
        } else {
            // This is a new hash and needs a new node.
            let new_id = self.next_node_id;
            self.next_node_id += 1;
            *slot = new_id;
            NodeId(new_id)
        }
    }
}

const FREE_TASK_ENTRY: ((NodeId, NodeId), NodeId) = ((NodeId::ZERO, NodeId::ZERO), NodeId::ZERO);

/// A variable-size leaky hash table for mapping pairs of `NodeIds` in the source BDDs to
/// their result `NodeIds`.
///
///  - It assumes left BDD is larger.
///  - It assumes value for task (0,0) is never saved.
///
pub struct TaskCache2 {
    /// Total number of entries in the table.
    capacity: NonZeroU64,
    /// Bits used to compute the address in a CPU-cache-friendly table block.
    block_mask: u64,
    /// Number of bits from the right pointer used for block entry computation.
    right_bits: u32,
    /// Number of nodes in the right BDD. Used for calculating the growth factor.
    right_len: u64,
    /// Number of collisions so-far. Used to determine if the cache should grow.
    collisions: u64,
    /// Actual hash-table.
    table: Vec<((NodeId, NodeId), NodeId)>,
}

impl TaskCache2 {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_node_count: u64, right_node_count: u64) -> TaskCache2 {
        TaskCache2 {
            capacity: NonZeroU64::new(left_node_count).unwrap(),
            block_mask: (1 << 14) - 1, // 16k entries, TODO: this should be dynamic
            right_bits: 0,             // Initially, only use left ids.
            right_len: right_node_count,
            collisions: 0,
            table: vec![FREE_TASK_ENTRY; left_node_count as usize],
        }
    }

    /// Get a reference to a particular slot. The slot must be valid in this cache.
    #[inline]
    pub unsafe fn read_slot(&self, slot: u64, task: (NodeId, NodeId)) -> NodeId {
        let entry = unsafe { self.table.get_unchecked(slot as usize) };
        if entry.0 == task {
            entry.1
        } else {
            NodeId::ZERO
        }
    }

    #[inline]
    pub unsafe fn write_slot(&mut self, slot: u64, task: (NodeId, NodeId), result: NodeId) {
        let entry = unsafe { self.table.get_unchecked_mut(slot as usize) };
        if Self::is_occupied(entry.0) {
            self.collisions += 1;
        }
        *entry = (task, result);
    }

    #[inline]
    pub fn should_grow(&self) -> bool {
        self.collisions > self.capacity.get()
    }

    pub fn grow(&mut self) {
        println!("Task cache is growing!"); // TODO: Remove
        let right_len_log_2 = 63 - self.right_len.leading_zeros();
        let right_len_log_log_2 = 31 - right_len_log_2.leading_zeros();
        // log2(right_len) rounded up to the next larger power of two
        let grow_by = 1u64 << right_len_log_log_2;
        let new_capacity = self.capacity.get() * grow_by;
        self.capacity = NonZeroU64::new(new_capacity).unwrap();
        self.right_bits += right_len_log_log_2;
        self.collisions = 0;
        let mut table = vec![FREE_TASK_ENTRY; new_capacity as usize];
        swap(&mut self.table, &mut table);
        for (task, result) in table {
            if Self::is_occupied(task) {
                let slot = self.find_slot(task);
                unsafe {
                    self.write_slot(slot, task, result);
                }
            }
        }
    }

    /// Compute the position of a particular id-pair in the cache table.
    ///
    /// Note that the position may become stale if the cache grows. However,
    /// since the cache cannot shrink and does not guarantee completeness, it
    /// is resilient to insertion into stale slots.
    #[inline]
    pub fn find_slot(&self, task: (NodeId, NodeId)) -> u64 {
        let (left, right) = task;

        let block_offset = {
            let left = u64::from(left).rotate_left(7).wrapping_mul(Self::SEED);
            let right = u64::from(right).wrapping_mul(Self::SEED);
            left.bitxor(right).bitand(self.block_mask)
        };

        let block_position = {
            let right_bits = self.right_bits;
            let left = u64::from(left).wrapping_shl(right_bits);
            let right = u64::from(right) & ((1 << right_bits) - 1);
            left.bitor(right)
        };

        (block_position + block_offset).rem(self.capacity)
    }

    #[inline]
    fn is_occupied(task: (NodeId, NodeId)) -> bool {
        // At least one of the values must be non-zero if this is a saved task.
        u64::from(task.0) | u64::from(task.1) != 0
    }
}
*/
