use crate::_bdd_u16::{StaticNodeCache, NodeU64, PointerU16};
use crate::{Variable, SEED64};

impl<const N: usize> StaticNodeCache<N> {

    pub fn new(capacity: usize) -> StaticNodeCache<N> {
        debug_assert!(N < usize::from(u16::MAX));
        debug_assert!(capacity <= N);
        StaticNodeCache {
            keys: [NodeU64::UNDEFINED; N],
            values: [PointerU16::UNDEFINED; N],
        }
    }

    /// Get a pointer to the node stored in this cache, or `PointerU16::UNDEFINED` if the cache
    /// does not contain this node.
    #[inline]
    pub fn read(&self, node: NodeU64) -> PointerU16 {
        let index = Self::hashed_index(node);
        unsafe {
            if *self.keys.get_unchecked(index) == node {
                *self.values.get_unchecked(index)
            } else {
                PointerU16::UNDEFINED
            }
        }
    }

    /// Save a node pointer into this cache, overwriting any colliding information already stored
    /// in the table.
    #[inline]
    pub fn write(&mut self, node: NodeU64, result: PointerU16) {
        let index = Self::hashed_index(node);
        unsafe {
            let key_cell = self.keys.get_unchecked_mut(index);
            let value_cell = self.values.get_unchecked_mut(index);
            *key_cell = node;
            *value_cell = result;
        }
    }

    #[inline]
    fn hashed_index(node: NodeU64) -> usize {
        (node.0.wrapping_mul(SEED64) % (N as u64)) as usize
    }

}