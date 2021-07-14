use crate::_bdd_u16::{PointerU16, StaticTaskCache};
use crate::SEED64;

impl<const N: usize> StaticTaskCache<N> {
    /// Creates a new `StaticTaskCache` with the given dimensions.
    ///
    /// *Panics (in debug) if `dimension_x * dimension_y >= u16::MAX`.*
    pub fn new(dimension_x: usize, dimension_y: usize) -> StaticTaskCache<N> {
        // This assertion also implies both dimensions are `< u16::MAX`.
        debug_assert!(N <= usize::from(u16::MAX));
        debug_assert!(dimension_x * dimension_y < usize::from(u16::MAX));
        StaticTaskCache {
            dimension_x,
            dimension_y,
            table: [PointerU16::UNDEFINED; N],
        }
    }

    /// Obtain a pointer saved in the cache, or `PointerU16::UNDEFINED` if the key is not
    /// present in the cache yet.
    #[inline]
    pub fn read(&self, x: PointerU16, y: PointerU16) -> PointerU16 {
        let table_index = self.table_index(x, y);
        unsafe { *self.table.get_unchecked(table_index) }
    }

    /// Save a pointer to the cache.
    #[inline]
    pub fn write(&mut self, x: PointerU16, y: PointerU16, result: PointerU16) {
        let table_index = self.table_index(x, y);
        unsafe {
            let cell = self.table.get_unchecked_mut(table_index);
            *cell = result;
        }
    }

    /// Compute a safe index into `self.table` based on two "input" pointers.
    #[inline]
    fn table_index(&self, x: PointerU16, y: PointerU16) -> usize {
        //self.check_bounds(x, y);
        // Assuming check-bounds passes, the conversion is safe and result also fits into u16.
        (y.0 as usize) * self.dimension_y + (x.0 as usize)
    }

    /// A validity check for input pointers - only performed during testing!
    #[inline]
    fn check_bounds(&self, x: PointerU16, y: PointerU16) {
        debug_assert!(usize::from(x.0) < self.dimension_x);
        debug_assert!(usize::from(y.0) < self.dimension_y);
    }
}
