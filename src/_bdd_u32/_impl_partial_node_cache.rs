use crate::_bdd_u32::PartialNodeCache;
use crate::{Variable, PointerPair, Pointer, SEED64};
use std::num::NonZeroU64;
use std::ops::Rem;

impl PartialNodeCache {

    pub fn new(capacity: usize) -> PartialNodeCache {
        PartialNodeCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            keys: vec![(Variable(u16::MAX), PointerPair(u64::MAX)); capacity],
            values: vec![Pointer::undef(); capacity],
        }
    }


    #[inline]
    pub fn read(&self, variable: Variable, pointers: PointerPair) -> Pointer {
        let index = self.hash_index(pointers);
        unsafe {
            if *self.keys.get_unchecked(index) == (variable, pointers) {
                *self.values.get_unchecked(index)
            } else {
                Pointer::undef()
            }
        }
    }

    #[inline]
    pub fn write(&mut self, variable: Variable, pointers: PointerPair, result: Pointer) {
        let index = self.hash_index(pointers);
        unsafe {
            let key_cell = self.keys.get_unchecked_mut(index);
            let value_cell = self.values.get_unchecked_mut(index);
            *key_cell = (variable, pointers);
            *value_cell = result;
        }
    }

    #[inline]
    fn hash_index(&self, pointers: PointerPair) -> usize {
        let hash = pointers.0.wrapping_mul(SEED64);
        hash.rem(self.capacity) as usize
    }

}