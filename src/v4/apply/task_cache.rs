use super::super::core::{NodeIndex, Bdd};
use crate::IntoIndex;
use std::ops::Rem;

type KeyValuePair = ((NodeIndex, NodeIndex), NodeIndex);

/// Used to avoid duplicate computation of the same tasks in the apply algorithm.
///
/// Each task is identified by two node indices from the left and right BDDs. The result is
/// a node index valid within the result BDD.
///
/// The hashing is based on a pseudo-local hash, where the assumption is that the left task id
/// forms a pseudo-growing sequence (due to the BDD being in pre-order), and the right task id
/// is used to generate a randomized index into a local block of candidate slots.
///
/// As the table becomes congested, the number of slots increases two-fold, and the low bits
/// from the right task id are used to form the pseudo-growing sequence of hash base values
/// as well.
pub struct TaskCache {
    /// The number of elements inserted into the cache so far. Used to determine whether
    /// we should grow the cache.
    elements: usize,
    /// The bit mask that should be applied to the block start
    bit_extension: u64,
    /// The actual capacity of the table when discounting the hash block size.
    capacity: u64,
    items: Vec<KeyValuePair>
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TaskCacheSlot(u64);

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

    pub fn new(left: &Bdd, right: &Bdd) -> TaskCache {
        debug_assert!(left.get_size() >= right.get_size());
        let capacity = (left.get_size() + Self::HASH_BLOCK).into_index();
        TaskCache {
            elements: 0,
            bit_extension: 0,
            capacity: left.get_size(),
            items: vec![Self::UNDEFINED_ENTRY; capacity]
        }
    }

    pub fn read(&self, task: (NodeIndex, NodeIndex)) -> Result<NodeIndex, TaskCacheSlot> {
        let slot = self.hashed_index(task);
        let slot_value = unsafe { self.items.get_unchecked(slot.into_index()) };
        if slot_value.0 == task {
            Ok(slot_value.1)
        } else {
            Err(slot)
        }
    }

    pub fn write(&mut self, slot: TaskCacheSlot, task: (NodeIndex, NodeIndex), result: NodeIndex) {
        let slot_value = unsafe { self.items.get_unchecked_mut(slot.into_index()) };
        *slot_value = (task, result);
        self.elements += 1;
    }

    pub fn grow_if_necessary(&mut self) {
        if self.elements >= 2 * self.items.len() {
            // Add one extra bit from the right task hash, and reset elements count.
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

    fn hashed_index(&self, task: (NodeIndex, NodeIndex)) -> TaskCacheSlot {
        let left: u64 = task.0.into();
        let right: u64 = task.1.into();
        let block_base = (left << (64 - self.bit_extension.leading_zeros())) | (right & self.bit_extension);
        let block_offset = right.wrapping_mul(Self::SEED).rem(Self::HASH_BLOCK);
        (block_base + block_offset).into()
    }

}