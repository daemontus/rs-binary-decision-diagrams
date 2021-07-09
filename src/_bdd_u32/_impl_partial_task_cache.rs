use crate::_bdd_u32::PartialTaskCache;
use crate::{Pointer, SEED64};
use std::ops::{Shl, Rem};
use std::num::NonZeroU64;

impl PartialTaskCache {

    pub fn new(capacity: usize) -> PartialTaskCache {
        PartialTaskCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            keys: vec![(Pointer::undef(), Pointer::undef()); capacity],
            values: vec![0; capacity],
        }
    }

    #[inline]
    pub fn read(&self, x: Pointer, y: Pointer) -> usize {
        let index = self.hash_index(x, y);
        unsafe {
            if *self.keys.get_unchecked(index) == (x,y) {
                *self.values.get_unchecked(index)
            } else {
                usize::MAX
            }
        }
    }

    #[inline]
    pub fn write(&mut self, x: Pointer, y: Pointer, queue_index: usize) {
        let index = self.hash_index(x, y);
        unsafe {
            let key_cell = self.keys.get_unchecked_mut(index);
            let value_cell = self.values.get_unchecked_mut(index);
            *key_cell = (x, y);
            *value_cell = queue_index;
        }
    }

    #[inline]
    fn hash_index(&self, x: Pointer, y: Pointer) -> usize {
        let hash = ((x.0 as u64).shl(32i32) + (y.0 as u64)).wrapping_mul(SEED64);
        hash.rem(self.capacity) as usize
    }

}