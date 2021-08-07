use super::PointerPair;
use crate::v2::NodeId;

/// **(internal)** A 32-bit version of the `Stack` used in the general `u48` algorithm.
/// Method documentation omitted when equivalent to the one on `u48` version.
///
/// The main difference is that the `left` and `right` pointers are only 32-bit, so we can
/// fit both of them into a single `u64`. There is one small caveat though: The `left` pointer
/// (which is expected to have a larger range) can go up to `2^32 - 1`, but the `right`
/// pointer can only go up to `2^31 - 1`. This is because we need to somehow differentiate
/// the results from tasks, and results can still theoretically in the worst case extend
/// to 48 bits. So we keep the top-most bit of the stack entry reserved as a flag whether
/// the item is a result or not.
pub(super) struct Stack {
    index_after_last: usize,
    items: Vec<PointerPair>,
}

impl Stack {
    pub fn new(variable_count: u16) -> Stack {
        let variable_count = usize::from(variable_count);
        let mut stack = Stack {
            index_after_last: 1,
            items: vec![PointerPair(0); 2 * variable_count + 2],
        };
        stack.items[0] = PointerPair::from(PointerPair::RESULT_MASK);
        stack
    }

    #[inline]
    pub fn has_last_entry(&self) -> bool {
        self.index_after_last == 2
    }

    #[inline]
    pub unsafe fn push_task_unchecked(&mut self, tasks: PointerPair) {
        debug_assert!(self.index_after_last < self.items.len());

        let entry = unsafe { self.items.get_unchecked_mut(self.index_after_last) };
        *entry = tasks;
        self.index_after_last += 1;
    }

    #[inline]
    pub fn has_result(&self) -> bool {
        debug_assert!(self.index_after_last > 1);

        unsafe {
            self.items
                .get_unchecked(self.index_after_last - 1)
                .is_result()
        }
    }

    #[inline]
    pub unsafe fn pop_results_unchecked(&mut self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 2);
        debug_assert!(self.items[self.index_after_last - 1].is_result());
        debug_assert!(self.items[self.index_after_last - 2].is_result());

        self.index_after_last -= 2;
        let x = unsafe { self.items.get_unchecked(self.index_after_last) };
        let y = unsafe { self.items.get_unchecked(self.index_after_last + 1) };
        (x.into_result(), y.into_result())
    }

    #[inline]
    pub unsafe fn peek_as_task_unchecked(&self) -> PointerPair {
        debug_assert!(self.index_after_last > 1);
        debug_assert!(!self.items[self.index_after_last - 1].is_result());

        unsafe { *self.items.get_unchecked(self.index_after_last - 1) }
    }

    #[inline]
    pub unsafe fn save_result_unchecked(&mut self, result: NodeId) -> bool {
        debug_assert!(self.index_after_last >= 2);
        debug_assert!(!self.items[self.index_after_last - 1].is_result());

        // This operation is safe because we have that dummy first
        // entry that gets accessed here if needed.
        let before_top_index = self.index_after_last - 2;
        let top_index = self.index_after_last - 1;
        let before_top = unsafe { self.items.get_unchecked_mut(before_top_index) };
        if before_top.is_result() {
            // entry[-2] is also a result - just replace the top
            unsafe {
                *self.items.get_unchecked_mut(top_index) = PointerPair::from_result(result);
            }
            true
        } else {
            // entry[-2] is a task - swap it on top
            let swap_on_top = *before_top;
            *before_top = PointerPair::from_result(result);
            unsafe {
                *self.items.get_unchecked_mut(top_index) = swap_on_top;
            }
            false
        }
    }
}
