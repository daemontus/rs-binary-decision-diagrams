use std::num::NonZeroU64;
use crate::v2::NodeId;
use crate::v2::_impl_::bdd::binary_operations::u32::PointerPair;
use std::cmp::max;
use std::convert::TryFrom;
use std::ops::{Rem, BitXor};

/// **(internal)** Task cache based on the general `u48` version. See the original
/// version for documentation comments.
///
/// The main difference is that the keys are now 32-bit pointers, saving
/// a bit of space and also some hashing time.
pub(super) struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<PointerPair>,
    values: Vec<NodeId>,
}

impl TaskCache {
    const HASH_BLOCK: u64 = 1 << 14;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_size: usize, right_size: usize) -> TaskCache {
        debug_assert!(left_size >= right_size);
        let capacity = max(left_size, right_size);
        TaskCache {
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            keys: vec![PointerPair(0); capacity],
            values: vec![NodeId::ZERO; capacity],
        }
    }

    #[inline]
    pub fn read(&self, tasks: PointerPair) -> NodeId {
        let index = self.hashed_index(tasks);
        unsafe {
            if *self.keys.get_unchecked(index) == tasks {
                *self.values.get_unchecked(index)
            } else {
                NodeId::UNDEFINED
            }
        }
    }

    #[inline]
    pub fn write(&mut self, tasks: PointerPair, result: NodeId) {
        let index = self.hashed_index(tasks);
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            let value = self.values.get_unchecked_mut(index);
            *key = tasks;
            *value = result;
        }
    }

    #[inline]
    pub fn prefetch(&self, tasks: PointerPair) {
        if cfg!(target_arch = "x86_64") {
            let index = self.hashed_index(tasks);
            unsafe {
                let key: *const PointerPair = self.keys.get_unchecked(index);
                let value: *const NodeId = self.values.get_unchecked(index);
                std::arch::x86_64::_mm_prefetch::<3>(key as *const i8);
                std::arch::x86_64::_mm_prefetch::<3>(value as *const i8);
            }
        }
    }

    #[inline]
    fn hashed_index(&self, tasks: PointerPair) -> usize {
        /*
            For some reason, the gods of hash functions don't want us to simplify
            this. The more-or-less viable alternative seems to be to xor the
            left right pointer and then multiply only once, but this produces
            a bit too many collisions for my liking (and only small perf.
            improvement), so I'm keeping this for now and we may change it down the line.
         */
        let (left, right) = tasks.unpack();
        let left_hash = u64::from(left).wrapping_mul(Self::SEED);
        let right_hash = u64::from(right).wrapping_mul(Self::SEED);
        let block_index = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        (left.0 + block_index).rem(self.capacity) as usize
        //left_hash.bitxor(right_hash).rem(self.capacity) as usize
    }
}
