use crate::_bdd_u16::{PointerU16, StaticStack};

impl<const N: usize> StaticStack<N> {
    pub fn new(variable_count: u16) -> StaticStack<N> {
        debug_assert!(usize::from(variable_count) <= N);
        StaticStack {
            index_after_last: 0,
            items: [(PointerU16::ZERO, PointerU16::ZERO); N],
        }
    }

    #[inline]
    pub fn push(&mut self, left: PointerU16, right: PointerU16) {
        unsafe {
            let cell = self.items.get_unchecked_mut(self.index_after_last);
            *cell = (left, right);
        }
        self.index_after_last += 1;
        debug_assert!(self.index_after_last < N)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index_after_last == 0
    }

    /// Return the element at the top of the stack. The result is undefined when the stack is empty!
    #[inline]
    pub fn peek(&self) -> (PointerU16, PointerU16) {
        debug_assert!(!self.is_empty());
        unsafe { *self.items.get_unchecked(self.index_after_last - 1) }
    }

    #[inline]
    pub fn pop(&mut self) {
        self.index_after_last -= 1;
    }
}
