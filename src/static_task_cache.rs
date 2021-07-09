use crate::Pointer;
use std::convert::TryFrom;

/// Static task cache implements a complete (Pointer, Pointer) -> Pointer mapping.
///
/// It is completely allocated on the stack and for this reason is only suitable for
/// operations where the largest pointer fits into `u16` (to save space).
struct StaticTaskCache<const N: usize> {
    dimension_x: usize,
    dimension_y: usize,
    table: [u16; N]
}

impl <const N: usize> StaticTaskCache<N> {

    pub fn new(x: usize, y: usize) -> StaticTaskCache<N> {
        debug_assert!(x * y < usize::from(u16::MAX));
        StaticTaskCache {
            dimension_x: x,
            dimension_y: y,
            table: [u16::MAX; N]
        }
    }

    #[inline]
    pub fn get(&self, x: Pointer, y: Pointer) -> Pointer {
        self.check_bounds(x, y);
        let table_index = self.table_index(x, y);
        let u16_pointer = unsafe { *self.table.get_unchecked(table_index) };
        todo!()
    }

    #[inline]
    fn table_index(&self, x: Pointer, y: Pointer) -> usize {
        // The conversion is safe assuming `check_bounds` passed
        (y.0 as usize) * self.dimension_y + (x.0 as usize)
    }

    #[inline]
    fn check_bounds(&self, x: Pointer, y: Pointer) {
        debug_assert!(usize::try_from(x.0).unwrap() < self.dimension_x);
        debug_assert!(usize::try_from(y.0).unwrap() < self.dimension_y);
    }

}