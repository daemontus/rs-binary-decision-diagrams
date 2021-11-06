use std::num::NonZeroU64;
use crate::v3::core::node_id::NodeId;
use std::ops::{BitXor, Rem};
use std::cmp::max;

pub struct TaskCache {
    collisions: u64,
    capacity: NonZeroU64,
    table: Vec<((NodeId, NodeId), NodeId)>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TaskCacheSlot(u64);

impl TaskCacheSlot {
    /// Convert slot back into index. It is valid because we only work on 64-bit systems.
    pub fn into_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u64> for TaskCacheSlot {
    fn from(value: u64) -> Self {
        TaskCacheSlot(value)
    }
}

impl From<TaskCacheSlot> for u64 {
    fn from(value: TaskCacheSlot) -> Self {
        value.0
    }
}

impl TaskCache {
    const HASH_BLOCK: u64 = 1 << 14;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_count: usize, right_count: usize) -> TaskCache {
        let initial_capacity = max(left_count, right_count);
        TaskCache {
            collisions: 0,
            capacity: NonZeroU64::new(initial_capacity as u64).unwrap(),
            table: vec![((NodeId::ZERO, NodeId::ZERO), NodeId::ZERO); initial_capacity],
        }
    }

    pub unsafe fn read_unchecked(&self, key: (NodeId, NodeId), slot: TaskCacheSlot) -> NodeId {
        let slot = unsafe { self.table.get_unchecked(slot.into_usize()) };
        if slot.0 == key {
            slot.1
        } else {
            NodeId::UNDEFINED
        }
    }

    pub unsafe fn write_unchecked(&mut self, key: (NodeId, NodeId), value: NodeId, slot: TaskCacheSlot) {
        let slot = unsafe { self.table.get_unchecked_mut(slot.into_usize()) };
        if slot.0 != (NodeId::ZERO, NodeId::ZERO) {
            self.collisions += 1;
        }
        *slot = (key, value);
    }

    pub fn check_rehash(&mut self) {
        if (self.collisions as usize) > self.table.len() {
            unimplemented!("Implement hash table reallocation.")
        }
    }

    pub fn find_slot(&self, key: (NodeId, NodeId)) -> TaskCacheSlot {
        let left: u64 = key.0.into();
        let right: u64 = key.1.into();
        let left_hash = left.rotate_left(7).wrapping_mul(Self::SEED);
        let right_hash = right.wrapping_mul(Self::SEED);
        let block_index = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        let block_start = left;
        TaskCacheSlot((block_start + block_index).rem(self.capacity))
    }

}

#[cfg(test)]
mod test {
    use crate::v3::core::ooo::task_cache::TaskCache;
    use crate::v3::core::node_id::NodeId;

    #[test]
    pub fn basic_task_cache_test() {
        let mut task_cache = TaskCache::new(10, 5);
        unsafe {
            let id_1 = NodeId::from(1u64);
            let id_2 = NodeId::from(2u64);
            let id_3 = NodeId::from(3u64);
            let id_4 = NodeId::from(4u64);
            let id_5 = NodeId::from(5u64);

            let slot_1_2 = task_cache.find_slot((id_1, id_2));
            assert_eq!(NodeId::UNDEFINED, task_cache.read_unchecked((id_1, id_2), slot_1_2));
            task_cache.write_unchecked((id_1, id_2), id_5, slot_1_2);
            assert_eq!(id_5, task_cache.read_unchecked((id_1, id_2), slot_1_2));
            task_cache.write_unchecked((id_1, id_2), id_4, slot_1_2);
            // This is not a "real" collision because in reality we only write each value once.
            assert_eq!(id_4, task_cache.read_unchecked((id_1, id_2), slot_1_2));
            assert_eq!(1, task_cache.collisions);

            let slot_1_3 = task_cache.find_slot((id_1, id_2));
            assert_eq!(NodeId::UNDEFINED, task_cache.read_unchecked((id_1, id_3), slot_1_3));
        }
    }

}