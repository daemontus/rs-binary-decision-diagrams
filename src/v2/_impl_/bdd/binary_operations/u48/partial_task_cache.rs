use std::num::NonZeroU64;
use crate::v2::NodeId;
use std::ops::{BitXor, Rem};

/// **(internal)** A partial hash map which saves the results of already processed tasks.
///
/// It is essentially a hash map which overwrites on collision to avoid costly branches.
/// It relies on the fact that task (0,0) should be always resolved using a lookup table
/// and will therefore never appear as a key in the cache. This way, we can start by
/// zeroing all the memory, which appears to be slightly faster on x86 for some reason.
pub(super) struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<(NodeId, NodeId)>,
    values: Vec<NodeId>,
}

impl TaskCache {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    /// **(internal)** Create a new `TaskCache` with the given (fixed!) capacity.
    pub fn new(capacity: usize) -> TaskCache {
        debug_assert!(capacity > 0);
        TaskCache {
            capacity: unsafe { NonZeroU64::new_unchecked(capacity as u64) },
            keys: vec![(NodeId::ZERO, NodeId::ZERO); capacity],
            values: vec![NodeId::ZERO; capacity],
        }
    }

    /// **(internal)** Read an entry from the cache. If the entry is not present, returns `NodeId::UNDEFINED`.
    #[inline]
    pub fn read(&self, left: NodeId, right: NodeId) -> NodeId {
        let index = self.hash(left, right);
        unsafe {
            if *self.keys.get_unchecked(index) == (left, right) {
                *self.values.get_unchecked(index)
            } else {
                NodeId::UNDEFINED
            }
        }
    }

    /// **(internal)** Write a new entry into the cache.
    #[inline]
    pub fn write(&mut self, left: NodeId, right: NodeId, result: NodeId) {
        let index = self.hash(left, right);
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            let value = self.values.get_unchecked_mut(index);
            *key = (left, right);
            *value = result;
        }
    }

    /// **(internal)** Prefetch the given entry if possible.
    #[inline]
    pub fn prefetch(&self, left: NodeId, right: NodeId) {
        if cfg!(target_arch = "x86_64") {
            let index = self.hash(left, right);
            unsafe {
                let key: *const (NodeId, NodeId) = self.keys.get_unchecked(index);
                let value: *const (NodeId) = self.values.get_unchecked(index);
                std::arch::x86_64::_mm_prefetch::<3>(key as *const i8);
                std::arch::x86_64::_mm_prefetch::<3>(value as *const i8);
            }
        }
    }

    /// **(internal)** A hash function inspired by Knuth and FxHash.
    #[inline]
    fn hash(&self, left: NodeId, right: NodeId) -> usize {
        let left = left.0.wrapping_mul(Self::SEED);
        let right = right.0.wrapping_mul(Self::SEED);
        left.bitxor(right).rem(self.capacity) as usize
    }
}

