//! Here we are building a completely out-of-order "BDD machine".
//!
//! The out-of-order algorithm is based on three (four?) core data structures.
//!
//! First is an in-order task stack. Here, tasks are created in the order in which they
//! appear in the DFS-preorder of the compound graph, and they are "issued" in the
//! DFS-postorder of the same graph. Tasks which resolve to constant nodes and tasks
//! which are located in task cache are retired immediately on the task stack and do not
//! move to further stages of the pipeline.
//!
//! However, if a task is not constant nor cached, once issued, it has an allocated place
//! in a reorder buffer. Locations in reorder buffer are allocated on the fly and do not
//! follow any specific order. However, they provide a fixed point of reference for the
//! spawning task where their results can be located. Hence we can continue with execution
//! assuming the results for these tasks will eventually be available. The spawning task
//! then frees entries from reorder buffer once it consumes their results.
//!
//! Finally, there are two connected queues which ensure tasks can be issued out of order.
//! Once a task has a place in the reorder buffer, it is placed in an execution queue. Here,
//! a task is waiting until both its dependencies are available (freeing the results from
//! ROB and copying the results into the task object). Once both dependencies are ready,
//! the task is "executed", meaning that the hash of the result node is computed and the
//! task moves into retirement queue. However, since the two queues are connected, no
//! memory actually changes place, it is simply a shift in the queue bounds.
//!
//! Here we will attempt to place the task into its candidate slot. If a duplicate node
//! is found, the result is placed in the ROB and task cache and free. In case of collision,
//! we update the task with a new candidate slot, or allocate a new slot if there are no
//! other candidates to compare with.
//!
//! Bonus points if you can figure out how to delay dependencies between each stage
//! such that each point in the pipeline has no data dependencies between each other and
//! only "commits" changes once per "cycle". But this is slightly more complicated and
//! I'm not sure it is worth the effort. Maybe as v2.
//!

use crate::v2::bench_fun::deps::NodeId;

/// Reorder buffer is super simple. It basically just allocates slots for node ids that
/// will be computed in the future. To track free cells, we use a linked list.
///
/// Note that the maximal number of tasks "in flight" is proportional to the depth of the
/// issue stack. The ROB size should be therefore at least the number of considered BDD
/// variables.
struct ReorderBuffer {
    buffer: Vec<u64>,
    next_free: u64,
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct RobSlot(u64);

impl ReorderBuffer {

    pub fn new(capacity: usize) -> ReorderBuffer {
        // Create a linked list starting in zero and going through all slots in the vector.
        let mut list = vec![0; capacity];
        for i in 0..list.len() {
            list[i] = (i + 1) as u64;
        }
        // Last element has no successor, so we set it to u64::MAX.
        list[list.len() - 1] = u64::MAX;
        ReorderBuffer {
            buffer: list,
            next_free: 0
        }
    }

    pub fn is_full(&self) -> bool {
        self.next_free == u64::MAX
    }

    /// Returns a reference to the next free ROB slot, and initializes said slot with an
    /// undefined value.
    ///
    /// **Safety:** The function can be called only when reorder buffer is not full.
    pub unsafe fn allocate_slot(&mut self) -> RobSlot {
        debug_assert!(!self.is_full());
        let slot_id = self.next_free;
        let slot_value = self.buffer.get_unchecked_mut(slot_id as usize);

        // Free slots are a linked list, hence slot value is either next free slot or undefined.
        self.next_free = *slot_value;
        // Erase the linked list pointer, meaning that this slot contains an unfinished task.
        *slot_value = u64::MAX;
        // Return a pointer to the newly allocated ROB slot.
        RobSlot(slot_id)
    }

    /// Free the value of the given `slot`.
    ///
    /// **Safety:** The given `slot` must be allocated in this ROB.
    pub unsafe fn free_slot(&mut self, slot: RobSlot) {
        let slot_id = slot.0;
        debug_assert!((slot_id as usize) < self.buffer.len()); // Check bounds.
        let slot_value = self.buffer.get_unchecked_mut(slot_id as usize);

        // Erase slot value and replace with pointer of next free slot.
        *slot_value = self.next_free;
        // Update next free value such that it points to this newly freed slot.
        self.next_free = slot_id;
    }

    /// Retrieve a `NodeId` that is stored in the given ROB `slot`. The value is
    /// `NodeId::UNDEFINED` if the result is not computed yet.
    ///
    /// **Safety:** The function returns an undefined `NodeId` if the slot is not allocated.
    pub unsafe fn get_slot_value(&self, slot: RobSlot) -> NodeId {
        NodeId(*self.buffer.get_unchecked(slot.0 as usize))
    }

    /// Update value of a specified ROB `slot` using the given `id`.
    ///
    /// **Safety:** The `slot` value must be allocated within this ROB.
    pub unsafe fn set_slot_value(&mut self, slot: RobSlot, id: NodeId) {
        let slot_value = self.buffer.get_unchecked_mut(slot.0 as usize);
        *slot_value = id.0;
    }

}

struct PendingTask {

}

struct ExecutionRetireQueue {
    queue: Vec<PendingTask>,
    execution_head: u64,
    execution_length: u64,
    retire_head: u64,
    retire_length: u64,
}

impl ExecutionRetireQueue {

    pub fn new(capacity: usize) -> ExecutionRetireQueue {

    }

}