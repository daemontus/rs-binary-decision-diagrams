pub struct UnsafeStack<T: Sized + Copy> {
    index_after_last: usize,
    items: Vec<T>
}

impl <T: Sized + Copy> UnsafeStack<T> {

    pub fn new(capacity: usize) -> UnsafeStack<T> {
        let mut items = Vec::with_capacity(capacity);
        unsafe { items.set_len(capacity); }
        UnsafeStack {
            items, index_after_last: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.index_after_last
    }

    pub fn is_empty(&self) -> bool {
        self.index_after_last == 0
    }

    pub fn peek(&mut self ) -> &mut T {
        unsafe { self.items.get_unchecked_mut(self.index_after_last - 1) }
    }

    pub fn peek_at(&mut self, offset: usize) -> &mut T {
        unsafe { self.items.get_unchecked_mut(self.index_after_last - offset) }
    }

    pub fn push(&mut self, item: T) {
        let slot = unsafe { self.items.get_unchecked_mut(self.index_after_last) };
        *slot = item;
        self.index_after_last += 1;
    }

    pub fn pop(&mut self) -> T {
        self.index_after_last -= 1;
        unsafe { *self.items.get_unchecked(self.index_after_last) }
    }

}
