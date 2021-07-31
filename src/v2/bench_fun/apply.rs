use crate::v2::bench_fun::deps::{NodeId, BddNode, VariableId};
use std::num::NonZeroU64;
use std::ops::{BitXor, Rem, Shl};
use std::cmp::max;
use std::convert::TryFrom;
use fxhash::hash;

pub struct NodeCache2 {
    capacity: NonZeroU64,
    pub index_after_last: usize,
    pub nodes: Vec<((u64, u64), u64)>,
    hashes: Vec<usize>,
}

impl NodeCache2 {
    const HASH_BLOCK: u64 = 1 << 14;
    pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(capacity: usize) -> NodeCache2 {
        let capacity = 2 * capacity;
        NodeCache2 {
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            index_after_last: 2,
            hashes: vec![0; capacity],
            nodes: vec![((0, 0), 0); capacity + (capacity / 2)],
        }
    }

    #[inline]
    pub fn prefetch(&self, node: &BddNode) {
        unsafe {
            if cfg!(target_arch = "x86_64") {
                let index = self.hash(&node);
                unsafe {
                    let hash: *const usize = self.hashes.get_unchecked(index);
                    //let key: *const (NodeId, NodeId) = self.keys.get_unchecked(index);
                    //let value: *const NodeId = self.values.get_unchecked(index);
                    std::arch::x86_64::_mm_prefetch::<3>(hash as *const i8);
                    //std::arch::x86_64::_mm_prefetch::<3>(value as *const i8);
                }
            }
        }
    }

    #[inline]
    pub fn ensure(&mut self, node: BddNode) -> NodeId {
        let hashed_position = self.hash(&node);
        unsafe {
            let packed = ((node.1.0 | (node.0.0 as u64).shl(48u64)), node.2.0);
            let cell_index = self.hashes.get_unchecked_mut(hashed_position);
            if *cell_index == 0 {
                // This hash was not seen before
                let id = self.index_after_last;
                let insert_at = self.nodes.get_unchecked_mut(id);
                *cell_index = id;
                self.index_after_last += 1;
                *insert_at = (packed, u64::MAX);
                return NodeId(id as u64);
            }

            let mut cell = self.nodes.get_unchecked_mut(*cell_index);
            if cell.0 == packed {
                return NodeId(*cell_index as u64);
            } else {
                let mut insert_at = cell.1;
                loop {
                    if insert_at != u64::MAX {
                        cell = self.nodes.get_unchecked_mut(insert_at as usize);
                        if cell.0 == packed {
                            return NodeId(insert_at)
                        }
                        insert_at = cell.1;
                    } else {
                        insert_at = self.index_after_last as u64;
                        cell.1 = insert_at as u64;
                        cell = self.nodes.get_unchecked_mut(insert_at as usize);
                        *cell = (packed, u64::MAX);
                        self.index_after_last += 1;
                        return NodeId(insert_at);

                    }
                }
            }
        }
    }


    #[inline]
    fn hash(&self, node: &BddNode) -> usize {
        let low_hash = node.low_link().0.wrapping_mul(Self::SEED);
        let high_hash = node.high_link().0.wrapping_mul(Self::SEED);
        let block_index = low_hash.bitxor(high_hash).rem(Self::HASH_BLOCK);
        let base = max(node.low_link().0, node.high_link().0);
        (base + block_index).rem(self.capacity) as usize
        //low_hash.bitxor(high_hash).rem(self.capacity) as usize
    }
}

pub struct NodeCache {
    pub collisions: usize,
    capacity: NonZeroU64,
    index_after_last: usize,
    pub next_id: u64,
    keys: Vec<(BddNode, NodeId, usize)>
}

impl NodeCache {
    const HASH_BLOCK: u64 = 1 << 14;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(capacity: usize) -> NodeCache {
        NodeCache {
            collisions: 0,
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            index_after_last: capacity,
            next_id: 2,
            keys: vec![(BddNode(VariableId(0), NodeId(0), NodeId(0)), NodeId(0), 0); capacity + (capacity / 2)]
        }
    }

    #[inline]
    pub fn ensure(&mut self, node: BddNode) -> NodeId {
        let hashed_position = self.hash(&node);
        unsafe {
            let mut cell = self.keys.get_unchecked_mut(hashed_position);
            if cell.0 == node {
                cell.1
            } else if cell.0.links() == (NodeId::ZERO, NodeId::ZERO) {  // empty spot
                let id = NodeId(self.next_id);
                self.next_id += 1;
                *cell = (node, id, usize::MAX);
                id
            } else {
                //let mut i = 0;
                //self.collisions += 1;
                // We have a collision :(
                let mut insert_at = cell.2;
                loop {
                    //i += 1;
                    if insert_at == usize::MAX {
                        cell.2 = self.index_after_last;
                        cell = self.keys.get_unchecked_mut(self.index_after_last);
                        let id = NodeId(self.next_id);
                        self.next_id += 1;
                        self.index_after_last += 1;
                        *cell = (node, id, usize::MAX);
                        //if i > 4 {
                        //    println!("I: {}", i);
                        //}
                        return id;
                    } else {
                        cell = self.keys.get_unchecked_mut(insert_at);
                        if cell.0 == node {
                            return cell.1;
                        }
                        insert_at = cell.2;
                    }
                }
            }
        }
    }

    #[inline]
    fn hash(&self, node: &BddNode) -> usize {
        //(hash(node) as u64).rem(self.capacity) as usize
        //let packed: u64 = node.low_link().0 | (node.variable().0 as u64).shl(48);
        let low_hash = node.low_link().0.wrapping_mul(Self::SEED);
        let high_hash = node.high_link().0.wrapping_mul(Self::SEED);
        low_hash.bitxor(high_hash).rem(self.capacity) as usize
        //let block_index = low_hash.bitxor(high_hash).rem(Self::HASH_BLOCK);
        //(u64::from(node.low_link()) + block_index).rem(self.capacity) as usize
    }
}

pub struct TaskCache {
    capacity: NonZeroU64,
    // If we put it together like this, the compiler can do assignment/move as vector operations
    // which turns out to be super fast...
    keys: Vec<((NodeId, NodeId), NodeId)>,
    //values: Vec<NodeId>,
}

impl TaskCache {
    const HASH_BLOCK: u64 = 1 << 14;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_size: usize, right_size: usize) -> TaskCache {
        debug_assert!(left_size >= right_size);
        let capacity = max(left_size, right_size);
        TaskCache {
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            keys: vec![((NodeId::ZERO, NodeId::ZERO), NodeId::ZERO); capacity],
            //values: vec![NodeId::ZERO; capacity],
        }
    }

    #[inline]
    pub fn read(&self, left: NodeId, right: NodeId) -> (NodeId, u64) {
        let index = self.hashed_index(left, right);
        unsafe {
            let cell = self.keys.get_unchecked(index);
            if cell.0 == (left, right) {
                (cell.1, index as u64)
            } else {
                (NodeId::UNDEFINED, index as u64)
            }
        }
    }

    #[inline]
    pub fn write_at(&mut self, left: NodeId, right: NodeId, index: u64, result: NodeId) {
        let index = index as usize;
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            //let value = self.values.get_unchecked_mut(index);
            *key = ((left, right), result);
            //*value = result;
        }
    }

    #[inline]
    pub fn write(&mut self, left: NodeId, right: NodeId, result: NodeId) {
        let index = self.hashed_index(left, right);
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            //let value = self.values.get_unchecked_mut(index);
            *key = ((left, right), result);
            //*value = result;
        }
    }

    #[inline]
    fn hashed_index(&self, left: NodeId, right: NodeId) -> usize {
        let left_hash = u64::from(left).rotate_left(7).wrapping_mul(Self::SEED);
        let right_hash = u64::from(right).wrapping_mul(Self::SEED);
        let block_index = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        let block_start = u64::from(left);
        unsafe {
            let pointer: *const ((NodeId, NodeId), NodeId) = self.keys.get_unchecked((block_start as usize) + 32);
            std::arch::x86_64::_mm_prefetch::<3>(pointer as *const i8);
        }
        (block_start + block_index).rem(self.capacity) as usize
    }
}


pub struct Stack {
    pub index_after_last: usize,
    pub items: Vec<(NodeId, NodeId)>,
}

impl Stack {
    pub fn new(variable_count: u16) -> Stack {
        let variable_count = usize::from(variable_count);
        let mut stack = Stack {
            index_after_last: 1,
            items: vec![(NodeId::ZERO, NodeId::ZERO); 3 * variable_count + 2],
        };
        stack.items[0] = (NodeId::UNDEFINED, NodeId::ZERO);
        stack
    }

    #[inline]
    pub fn has_last_entry(&self) -> bool {
        self.index_after_last == 2
    }

    #[inline]
    pub unsafe fn push_result(&mut self, result: NodeId) {
        debug_assert!(self.index_after_last < self.items.len());

        unsafe { *self.items.get_unchecked_mut(self.index_after_last) = (NodeId::UNDEFINED, result); }
        self.index_after_last += 1;
    }

    #[inline]
    pub unsafe fn push_task_unchecked(&mut self, left: NodeId, right: NodeId) {
        debug_assert!(self.index_after_last < self.items.len());

        unsafe { *self.items.get_unchecked_mut(self.index_after_last) = (left, right); }
        self.index_after_last += 1;
    }

    #[inline]
    pub fn has_result(&self) -> bool {
        debug_assert!(self.index_after_last > 1);

        let top_left = unsafe { self.items.get_unchecked(self.index_after_last - 1).0 };
        top_left.is_undefined()
    }

    #[inline]
    pub unsafe fn pop_results_unchecked(&mut self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 2);
        debug_assert!(self.items[self.index_after_last - 1].0.is_undefined());
        debug_assert!(self.items[self.index_after_last - 2].0.is_undefined());

        self.index_after_last -= 2;
        let x = unsafe { self.items.get_unchecked(self.index_after_last).1 };
        let y = unsafe { self.items.get_unchecked(self.index_after_last + 1).1 };
        (x, y)
    }

    #[inline]
    pub unsafe fn pop_as_task_unchecked(&mut self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 1);
        debug_assert!(!self.items[self.index_after_last - 1].0.is_undefined());

        self.index_after_last -= 1;
        unsafe { *self.items.get_unchecked(self.index_after_last) }
    }


    #[inline]
    pub unsafe fn peek_as_task_unchecked(&self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 1);
        debug_assert!(!self.items[self.index_after_last - 1].0.is_undefined());

        unsafe { *self.items.get_unchecked(self.index_after_last - 1) }
    }

    #[inline]
    pub unsafe fn save_result_unchecked(&mut self, result: NodeId) -> bool {
        debug_assert!(self.index_after_last >= 2);
        debug_assert!(!self.items[self.index_after_last - 1].0.is_undefined());

        // This operation is safe because we have that dummy first
        // entry that gets accessed here if needed.
        let before_top_index = self.index_after_last - 2;
        let top_index = self.index_after_last - 1;
        let before_top = unsafe { self.items.get_unchecked_mut(before_top_index) };

        if before_top.0.is_undefined() {
            // entry[-2] is also a result - just replace the top
            unsafe {
                *self.items.get_unchecked_mut(top_index) = (NodeId::UNDEFINED, result);
            }
            true
        } else {
            // entry[-2] is a task - swap it on top
            let swap_on_top = *before_top;
            *before_top = (NodeId::UNDEFINED, result);
            unsafe {
                *self.items.get_unchecked_mut(top_index) = swap_on_top;
            }
            false
        }
    }
}