use crate::v2::NodeId;

/// **(internal)** A stack that keeps track of tasks that still need to be completed or the results
/// which have not been used yet. Each entry has either two valid left/right `NodeId` pointers,
/// or a single `NodeId::UNDEFINED` and a valid result pointer.
///
/// The special feature of the stack is that when you replace the top task with a result value,
/// it will automatically swap with an entry underneath, if that entry is not result as well.
/// This mechanism ensures that if the top entry is a result, we know that the entry underneath
/// is a result as well and we can finish the task that spawned them.
pub(super) struct Stack {
    index_after_last: usize,
    items: Vec<(NodeId, NodeId)>,
}

impl Stack {

    /// **(internal)** Create a new stack with a sufficient capacity for a "coupled DFS" over
    /// `Bdds` with depth bounded by `variable_count`.
    pub fn new(variable_count: u16) -> Stack {
        let variable_count = usize::from(variable_count);
        let mut stack = Stack {
            index_after_last: 1,
            // In a standard "coupled DFS" algorithm, the stack can never be larger than
            // 2 * the number of variables in the Bdd.
            items: vec![(NodeId::ZERO, NodeId::ZERO); 2 * variable_count + 2],
        };
        // A "fake" first entry ensures that even when the last task finishes, we can safely
        // check the predecessor. It does not count into the length of the stack.
        // Since the fake entry is a result, they will not swap.
        stack.items[0] = (NodeId::UNDEFINED, NodeId::ZERO);
        stack
    }

    /// **(internal)** Returns `true` if the stack has only one entry. This is actually the
    /// terminating condition for the "coupled DFS" search, because in a do-while loop,
    /// the last entry must be a result.
    #[inline]
    pub fn has_last_entry(&self) -> bool {
        self.index_after_last == 2
    }

    /// **(internal)** Create a new task entry on the stack.
    ///
    /// *Precondition:* The capacity of the stack is sufficient (should be trivially satisfied
    /// in coupled DFS search).
    #[inline]
    pub unsafe fn push_task_unchecked(&mut self, left: NodeId, right: NodeId) {
        debug_assert!(self.index_after_last < self.items.len());

        unsafe { *self.items.get_unchecked_mut(self.index_after_last) = (left, right) }
        self.index_after_last += 1;
    }

    /// **(internal)** Returns `true` if the top entry is a result.
    ///
    /// *Precondition:* The stack is not empty, which is satisfied if items are popped correctly.
    #[inline]
    pub fn has_result(&self) -> bool {
        debug_assert!(self.index_after_last > 1);

        let top_left = unsafe { self.items.get_unchecked(self.index_after_last - 1).0 };
        top_left.is_undefined()
    }

    /// **(internal)** Pop two entries off the stack, interpreting them as result ids.
    ///
    /// *Precondition:* The two top entries exist and are results.
    #[inline]
    pub unsafe fn pop_results_unchecked(&mut self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 2);
        debug_assert!(self.items[self.index_after_last - 1].0.is_undefined());
        debug_assert!(self.items[self.index_after_last - 2].0.is_undefined());

        self.index_after_last -= 2;
        let x = unsafe { self.items.get_unchecked(self.index_after_last).1 };
        let y = unsafe { self.items.get_unchecked(self.index_after_last + 1).1 };
        (x, y)
    }

    /// **(internal)** Get the top entry without popping it, interpreting it as a task.
    ///
    /// *Precondition:* The top entry exists and is a task entry. The entry existence requirement
    /// should be satisfied if the stack is popped correctly, but the fact that the entry is
    /// a task is not guaranteed by the stack.
    #[inline]
    pub unsafe fn peek_as_task_unchecked(&self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 1);
        debug_assert!(!self.items[self.index_after_last - 1].0.is_undefined());

        unsafe { *self.items.get_unchecked(self.index_after_last - 1) }
    }

    /// **(internal)** Try to replace the top of the stack with a result entry. If the next entry
    /// below the top one is a task, then the result is put at that position and the task
    /// becomes the top.
    ///
    /// Return `true` if the top of the stack is now a result, or `false` if a task entry has
    /// been swapped on top instead.
    ///
    /// *Precondition:* There is at least one entry on the stack and the top entry is a task.
    #[inline]
    pub unsafe fn save_result_unchecked(&mut self, result: NodeId) -> bool {
        debug_assert!(self.index_after_last >= 2);
        debug_assert!(!self.items[self.index_after_last - 1].0.is_undefined());

        // This operation is safe because we have that dummy first
        // entry that gets accessed here if needed.
        let before_top_index = self.index_after_last - 2;
        let top_index = self.index_after_last - 1;
        let before_top = unsafe { self.items.get_unchecked_mut(before_top_index) };
        if before_top.0.is_undefined() {
            // entry[-2] is also a result - just replace the top
            unsafe {
                *self.items.get_unchecked_mut(top_index) = (NodeId::UNDEFINED, result);
            }
            true
        } else {
            // entry[-2] is a task - swap it on top
            let swap_on_top = *before_top;
            *before_top = (NodeId::UNDEFINED, result);
            unsafe {
                *self.items.get_unchecked_mut(top_index) = swap_on_top;
            }
            false
        }
    }
}
