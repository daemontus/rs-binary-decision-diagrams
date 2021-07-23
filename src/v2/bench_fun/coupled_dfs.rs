use crate::v2::bench_fun::deps::NodeId;
use std::ops::{BitXor, Rem};
use std::num::NonZeroU64;
use std::cmp::max;
use std::convert::TryFrom;

pub struct TaskSet {
    capacity: NonZeroU64,
    keys: Vec<(NodeId, NodeId)>,
}

impl TaskSet {
    const HASH_BLOCK: u64 = 1 << 14;
    pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_size: usize, right_size: usize) -> TaskSet {
        debug_assert!(left_size >= right_size);
        let capacity = max(left_size, right_size);
        TaskSet {
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            keys: vec![(NodeId::ZERO, NodeId::ZERO); capacity],
        }
    }

    /// Return true if item was inserted.
    #[inline]
    pub fn ensure(&mut self, left: NodeId, right: NodeId) -> bool {
        let index = self.hashed_index(left, right);
        unsafe {
            let cell = self.keys.get_unchecked_mut(index);
            if *cell == (left, right) {
                false
            } else {
                *cell = (left, right);
                true
            }
        }
    }

    #[inline]
    fn hashed_index(&self, left: NodeId, right: NodeId) -> usize {
        // Shift prevents collisions on queries with high number of left == right tasks.
        let left_hash = u64::from(left).rotate_left(7).wrapping_mul(Self::SEED);
        let right_hash = u64::from(right).wrapping_mul(Self::SEED);
        let block_index: u64 = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        let block_start: u64 = u64::from(left);// + u64::from(right).shr(10);
        (block_start + block_index).rem(self.capacity) as usize
        //left_hash.bitxor(right_hash).rem(self.capacity) as usize
    }

}

pub struct UnsafeStack {
    index_after_last: usize,
    items: Vec<(NodeId, NodeId)>
}

impl UnsafeStack {

    pub fn new(variable_count: u16) -> UnsafeStack {
        let capacity = 2 * usize::from(variable_count) + 2;
        let mut items = Vec::with_capacity(capacity);
        unsafe { items.set_len(items.capacity()); }
        UnsafeStack {
            items,
            index_after_last: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, left: NodeId, right: NodeId) {
        if left.is_terminal() && right.is_terminal() {
            return;
        }
        unsafe {
            let cell = self.items.get_unchecked_mut(self.index_after_last);
            *cell = (left, right);
            self.index_after_last += 1;
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index_after_last == 0
    }

    #[inline]
    pub fn pop(&mut self) -> (NodeId, NodeId) {
        self.index_after_last -= 1;
        unsafe { *self.items.get_unchecked_mut(self.index_after_last) }
    }

}
