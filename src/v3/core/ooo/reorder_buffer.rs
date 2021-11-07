use crate::v3::core::node_id::NodeId;

/// `ReorderBuffer` (ROB) keeps track of outstanding dependencies of "in flight" tasks.
///
/// Each task that exits `TaskStack` has a slot in the ROB that will be used to store the
/// output of said task. That is, once a task X is finished, it's result is written into the ROB.
/// From there, it is later picked up by its parent task Y once Y starts executing. At that
/// point the ROB slot is freed and can be used for another task that is exiting the stack.
///
/// Tasks in ROB don't have any particular order. Initially, the ROB is empty and the slots form
/// a linked list of cells ready for allocation (accessed via the `next_free` property). Once
/// a slot is allocated, it is "detached" from this list and its value is cleared, only to be
/// eventually set to the result of the corresponding pending task. Finally, once the slot is
/// freed, it is again appended to the `next_free` list as its head. This means that the slots
/// are allocated more or less in a stack-like fashion, but the order can eventually deviate
/// substantially since some tasks will be deallocated out of order.
///
/// We'd like to keep the buffer as small as possible, however, it always needs to have at least
/// as many items as the `TaskStack`. That is because every task on the task stack can in the
/// worst case have one child task with a result in the ROB, and one that is decoded but not
/// executing yet. However, this assumes that in the worst case, we are fine with a complete
/// pipeline stall where tasks need to be basically executed in order to free up some ROB space.
/// As such, it is better to over-spec the ROB a little bit, especially since its stack-like
/// behaviour means the unused space will never pollute any caches in a major way, and the
/// initialization cost is minimal. Hence a 2 * (H(A) + H(B)) seems like a good recommended
/// ROB size.
///
/// Due to this fact, we assume that ROB will never grow beyond `2^32` elements and use `u32`
/// for indexing slots. This is beneficial mainly in combination with other data structures
/// that need to store the ROB slot id together with a BDD variable which is also `u32`,
/// so together they are an 8-byte value.
///
/// Note that with the recommended ROB size and `u32` variables, ROB indices can in theory
/// overflow `u32`. However, we are not expecting that this happens in practice, because a BDD with
/// `u32::MAX` height is still probably too large for any reasonable computer. The option to have
/// that many variables is mainly to enable aliases and substitutions where not every BDD uses
/// every variable. However, we should check for this overflow before creating the ROB!!
pub struct ReorderBuffer {
    /// If the slot is free, it points to another free slot (or u32::MAX if it is last).
    /// If it is allocated, it is either `NodeId::UNDEFINED`, or the task result.
    buffer: Vec<u64>,
    /// A pointer to the first free slot in the ROB.
    next_free: RobSlot,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RobSlot(u32);

impl RobSlot {
    pub const UNDEFINED: RobSlot = RobSlot(u32::MAX);

    pub fn into_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for RobSlot {
    fn from(value: u32) -> Self {
        RobSlot(value)
    }
}

impl From<RobSlot> for u32 {
    fn from(value: RobSlot) -> Self {
        value.0
    }
}

impl ReorderBuffer {

    pub fn new(capacity: usize) -> ReorderBuffer {
        // Create a linked list starting in zero and going through all slots in the vector.
        let mut list = vec![0; capacity];
        for i in 0..list.len() {
            list[i] = (i + 1) as u64;
        }
        // Last element has no successor, so we set it to u64::MAX.
        let last_index = list.len() - 1;
        list[last_index] = u64::MAX;
        ReorderBuffer {
            buffer: list,
            next_free: RobSlot(0)
        }
    }

    pub fn is_full(&self) -> bool {
        self.next_free == RobSlot::UNDEFINED
    }

    /// Returns a reference to the next free ROB slot, and initializes said slot with an
    /// undefined value.
    ///
    /// **Safety:** The function can be called only when reorder buffer is not full.
    pub unsafe fn allocate_slot(&mut self) -> RobSlot {
        debug_assert!(!self.is_full());
        let slot_id = self.next_free;
        let slot_value = unsafe { self.buffer.get_unchecked_mut(slot_id.into_usize()) };

        // Free slots are a linked list, hence slot value is either next free slot or undefined.
        self.next_free = RobSlot::from(*slot_value as u32);
        // Erase the linked list pointer, meaning that this slot contains an unfinished task.
        *slot_value = u64::MAX;
        // Return a pointer to the newly allocated ROB slot.
        slot_id
    }

    /// Free the value of the given `slot`.
    ///
    /// **Safety:** The given `slot` must be allocated in this ROB.
    pub unsafe fn free_slot(&mut self, slot: RobSlot) {
        let slot_id: u32 = slot.into();
        debug_assert!((slot_id as usize) < self.buffer.len()); // Check bounds.
        let slot_value = unsafe { self.buffer.get_unchecked_mut(slot_id as usize) };

        // Erase slot value and replace with pointer of next free slot.
        *slot_value = u64::from(u32::from(self.next_free));
        // Update next free value such that it points to this newly freed slot.
        self.next_free = slot;
    }

    /// Retrieve a `NodeId` that is stored in the given ROB `slot`. The value is
    /// `NodeId::UNDEFINED` if the result is not computed yet.
    ///
    /// **Safety:** The function returns an undefined `NodeId` if the slot is not allocated.
    pub unsafe fn get_slot_value(&self, slot: RobSlot) -> NodeId {
        NodeId::from(unsafe { *self.buffer.get_unchecked(slot.0 as usize) })
    }

    /// Update value of a specified ROB `slot` using the given `id`.
    ///
    /// **Safety:** The `slot` value must be allocated within this ROB.
    pub unsafe fn set_slot_value(&mut self, slot: RobSlot, id: NodeId) {
        let slot_value = unsafe { self.buffer.get_unchecked_mut(slot.0 as usize) };
        *slot_value = id.into();
    }

}


#[cfg(test)]
mod test {
    use crate::v3::core::node_id::NodeId;
    use crate::v3::core::ooo::reorder_buffer::ReorderBuffer;

    #[test]
    pub fn basic_rob_test() {
        unsafe {
            let mut rob = ReorderBuffer::new(3);
            assert!(!rob.is_full());
            let slot_1 = rob.allocate_slot();
            let slot_2 = rob.allocate_slot();
            assert!(!rob.is_full());
            rob.set_slot_value(slot_1, 3u64.into());
            rob.set_slot_value(slot_2, 5u64.into());
            assert_eq!(NodeId::from(3u64), rob.get_slot_value(slot_1));
            assert_eq!(NodeId::from(5u64), rob.get_slot_value(slot_2));
            let slot_3 = rob.allocate_slot();
            assert_ne!(slot_1, slot_3);
            assert_ne!(slot_2, slot_3);
            assert!(rob.is_full());
            rob.free_slot(slot_2);
            assert!(!rob.is_full());
            let slot_2_again = rob.allocate_slot();
            assert_eq!(slot_2, slot_2_again);
        }
    }

}