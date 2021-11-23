use super::super::core::NodeIndex;
use crate::IntoIndex;
use std::ops::Rem;

type KeyValuePair = ((NodeIndex, NodeIndex), NodeIndex);

/// Used to avoid duplicate computation of the same tasks in the apply algorithm.
///
/// Each task is identified by two node indices from the left and right BDDs. The result is
/// a node index valid within the output BDD. To avoid bottlenecks in the main algorithm by
/// the means of failed branch prediction and cache misses, the table is implemented as leaky,
/// meaning that the elements in it are overwritten on collision.
///
/// The hashing algorithms exploits the assumption that during computation, BDDs should be sorted
/// in DFS pre-order. As such, the absolute value of the explored indices should be decreasing
/// (root has the largest index, subsequent nodes are smaller). The the hash itself is then split
/// into two parts: *base* and *offset*.
///
/// As the base value, we simply use the index into the left BDD. Assuming the left BDD is the
/// larger of the two arguments, this provides relatively good granularity and locality. In case
/// the table is congested and needs to grow, we additionally shift in multiple bits from the
/// right index to obtain a larger base. Therefore, as the table grows, this procedure converges
/// to a normal `m x n` table where the size of the right BDD is rounded up to the nearest `2^k`.
///
/// To compute the offset, we use standard Knuth hashing via constant multiplication. However, the
/// range of the offset is limited to only a small interval (`2^13` at the moment). As such, once
/// it is added to the base value, it will add a certain amount of pseudo-random noise to it.
/// This noise will significantly reduce collisions, but it cannot make the hash diverge too much
/// from the expected base value and should therefore preserve its locality.
pub struct TaskCache {
    /// The number of elements inserted into the cache so far. Used to determine whether
    /// we should grow the cache.
    elements: u64,
    /// The bit mask that determines how many bits will be shifted into the hash base from the
    /// right node index.
    bit_extension: u64,
    /// The actual capacity of the table when discounting the hash block size.
    capacity: u64,
    items: Vec<KeyValuePair>
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TaskCacheSlot(u64);

impl TaskCacheSlot {
    pub const UNDEFINED: TaskCacheSlot = TaskCacheSlot(u64::MAX);
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

impl IntoIndex for TaskCacheSlot {
    fn into_index(self) -> usize {
        self.0.into_index()
    }
}

impl TaskCache {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    const HASH_BLOCK: u64 = 1 << 13;
    const UNDEFINED_ENTRY: KeyValuePair = ((NodeIndex::UNDEFINED, NodeIndex::UNDEFINED), NodeIndex::UNDEFINED);

    pub fn new(initial_capacity: u64) -> TaskCache {
        // By growing the cash capacity by the hash block size, we ensure that modulo is not needed
        // on the computed hashed indices.
        let actual_capacity = initial_capacity + Self::HASH_BLOCK;
        TaskCache {
            elements: 0,
            bit_extension: 0,
            capacity: initial_capacity,
            items: vec![Self::UNDEFINED_ENTRY; actual_capacity.into_index()]
        }
    }

    #[inline]
    pub fn read(&self, task: (NodeIndex, NodeIndex)) -> (NodeIndex, TaskCacheSlot) {
        // Note that this has been tested as slightly faster than a version that returns
        // the values as a Result<NodeIndex, TaskCacheSlot>.
        let slot = self.hashed_index(task);
        let slot_value = unsafe { self.items.get_unchecked(slot.into_index()) };
        if slot_value.0 == task {
            (slot_value.1, slot)
        } else {
            (NodeIndex::UNDEFINED, slot)
        }
    }

    #[inline]
    pub fn write(&mut self, slot: TaskCacheSlot, task: (NodeIndex, NodeIndex), result: NodeIndex) {
        let slot_value = unsafe { self.items.get_unchecked_mut(slot.into_index()) };
        *slot_value = (task, result);
        self.elements += 1;
    }
/*
    pub fn grow_if_necessary(&mut self) {
        if self.elements >= 2 * self.capacity {
            // Add one extra bit into the right index bit mask, and reset element count.
            self.bit_extension = (self.bit_extension << 1) | 1;
            self.elements = 0;
            // Create a new table and swap it with the current one.
            self.capacity = self.capacity * 2;
            let mut items = vec![Self::UNDEFINED_ENTRY; (self.capacity + Self::HASH_BLOCK).into_index()];
            std::mem::swap(&mut items, &mut self.items);
            // Rehash all values in the table.
            for (key, value) in items {
                if !value.is_undefined() {
                    let slot = self.hashed_index(key);
                    self.write(slot, key, value);
                }
            }
        }
    }
*/
    fn hashed_index(&self, task: (NodeIndex, NodeIndex)) -> TaskCacheSlot {
        let (left, right) = (u64::from(task.0), u64::from(task.1));
        let right_hash = right.wrapping_mul(Self::SEED);
        let block_offset = right_hash.rem(Self::HASH_BLOCK);
        let shift_bits = 64 - self.bit_extension.leading_zeros();
        let block_base: u64 = (left << shift_bits) | (right & self.bit_extension);
        (block_base + block_offset).into()
    }

}