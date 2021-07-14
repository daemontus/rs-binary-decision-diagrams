use crate::_bdd_u32::PartialNodeCache;
use crate::{Bdd, Pointer, PointerPair, Variable, SEED64};
use std::num::NonZeroU64;
use std::ops::Rem;

impl PartialNodeCache {
    pub fn new(capacity: usize) -> PartialNodeCache {
        PartialNodeCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            //keys: vec![(Variable(0), PointerPair(0)); capacity],
            values: vec![Pointer::zero(); capacity],
        }
    }

    pub fn clear(&mut self) {
        /*for i in self.values.iter_mut() {
            *i = Pointer::undef();//(Variable(0), PointerPair(0));
        }*/
    }

    #[inline]
    pub fn read(&self, variable: Variable, pointers: PointerPair, result: &Bdd) -> Pointer {
        let index = self.hash_index(pointers);
        unsafe {
            let pointer_cell = self.values.get_unchecked(index);
            if (pointer_cell.0 as usize) < result.nodes.len() {
                let pointer = pointer_cell.0 as usize;
                let bdd_pointer = result.nodes.get_unchecked(pointer);
                if *bdd_pointer == (0, 0, variable, pointers) {
                    return *pointer_cell;
                }
            }

            return Pointer::undef();
            /*if *self.keys.get_unchecked(index) == (variable, pointers) {
                *self.values.get_unchecked(index)
            } else {
                Pointer::undef()
            }*/
        }
    }

    #[inline]
    pub fn write(&mut self, variable: Variable, pointers: PointerPair, result: Pointer) {
        let index = self.hash_index(pointers);
        unsafe {
            //let key_cell = self.keys.get_unchecked_mut(index);
            let value_cell = self.values.get_unchecked_mut(index);
            //*key_cell = (variable, pointers);
            *value_cell = result;
        }
    }

    #[inline]
    fn hash_index(&self, pointers: PointerPair) -> usize {
        let hash = pointers.0.wrapping_mul(SEED64);
        hash.rem(self.capacity) as usize
    }
}
