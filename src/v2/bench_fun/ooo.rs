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

use std::cmp::min;
use crate::v2::bench_fun::apply::{NodeCache, TaskCache};
use crate::v2::bench_fun::deps::{Bdd, BddNode, NodeId, VariableId};
use crate::v2::bench_fun::{ID_MASK, VARIABLE_MASK};

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

impl RobSlot {

    pub fn from_masked(value: u64) -> RobSlot {
        RobSlot(value ^ SLOT_MASK)
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

/// Pending tasks are stored in the execution-retire queue and contain all the necessary
/// data to successfully complete a task (note that depending on the current state of the
/// task, some values may be undefined or change semantics).
///
///  - Task cache slot is the place in the task cache where the result will be placed. This value
///    should be assigned during the "issue" phase and so is constant once the task reaches
///    the execution queue.
///  - Variable is the identifier of a BDD variable which the result should condition on. This
///    value is also determined during "issue" and will not change.
///  - ROB slot identifies a slot in the ROB where the result of this task will be placed. Also
///    constant once task is issued.
///  - Node cache slot is initially undefined. It is determined during the "execution" phase.
///    Afterwards, it can be modified during "retire" if it collides with an existing node.
///  - Result low/high contain either a valid `NodeId`, or an `RobSlot`. The `NodeId` will be
///    initially present if the task has been resolved during decode. Otherwise, an `RobSlot`
///    of an executing task will be given instead. If the value is a `NodeId`, it is constant.
///    In case it is an `RobSlot`, before execution, we check the value in the ROB and if it
///    is available, we replace the slot value with the final value, and we free the ROB slot.
///
/// Overall the structure has 40 bytes, which is quite a lot, but there really isn't much that
/// we can do about it.
///
///
#[derive(Clone, Default)]
struct PendingTask {
    rob_slot: u32,
    variable: u32,
    result_low: u64,
    result_high: u64,
    task_cache_slot: u64,
    node_cache_slot: u64,
}

impl PendingTask {

    pub fn has_low_result(&self) -> bool {
        self.result_low & SLOT_MASK == 0
    }

    pub fn has_high_result(&self) -> bool {
        self.result_high & SLOT_MASK == 0
    }

}

/// Execution queue keeps track of tasks that do not have their dependencies resolved yet.
/// Once both results of the task are known, its candidate node cache slot can be computed,
/// and the task can be forwarded to the retire queue.
///
/// The execution queue has a head that points to the first element of the queue, and a tail
/// which points to the first free slot for inserting new tasks. The queue is empty if
/// head == tail. The retire queue has its own head which points to the first element, with
/// execution head serving as the tail of the retire queue. That is, retire queue is empty if
/// retire head == execution head. Finally, the whole queue is full if execution tail + 1 == retire
/// head. Technically, this leaves one empty slot that will never be used, but
/// if we used tail == retire head as the condition, then this is also true for a completely
/// empty queue.
///
/// Note that internally, the queue actually goes from "right" to "left", because in this case,
/// advancing the queue can be done without the danger of index underflow.
///
struct ExecutionRetireQueue<const LEN: usize> {
    queue: [PendingTask; LEN],
    retire_head: usize,
    execution_head: usize,
    execution_tail: usize,
}

impl<const LEN: usize> ExecutionRetireQueue<LEN> {

    pub fn new() -> ExecutionRetireQueue<LEN> {
        ExecutionRetireQueue {
            queue: [PendingTask::default(); LEN],
            retire_head: 0,
            execution_head: 0,
            execution_tail: 0,
        }
    }

    /// Checks whether this execution-retire queue has free slots into which new tasks
    /// can be enqueued.
    pub fn is_full(&self) -> bool {
        (self.execution_tail + 1) % LEN == self.retire_head
    }

    pub fn is_empty(&self) -> bool {
        self.execution_tail == self.execution_head && self.execution_head == self.retire_head
    }

    /// Return true if the queue contains at least one task in the execution queue.
    pub fn can_execute(&self) -> bool {
        self.execution_head != self.execution_tail
    }

    /// Return true if the queue contains at least one task in the retire queue.
    pub fn can_retire(&self) -> bool {
        self.retire_head != self.execution_head
    }

    /// Add a new task into this queue, that will be marked for execution.
    ///
    /// **Safety:** The method can be only called on a queue that is not full!
    pub unsafe fn enqueue_for_execution(&mut self, task: PendingTask) {
        debug_assert!(!self.is_full());
        let slot = self.queue.get_unchecked_mut(self.execution_tail);
        *slot = task;
        self.execution_tail = (self.execution_tail + 1) % LEN
    }

    /// Obtain the reference to the task that should be executed next.
    ///
    /// **Safety:** If the method is called on an empty queue, the resulting reference is valid,
    /// but its contents are undefined.
    pub unsafe fn execute_task_reference(&mut self) -> &mut PendingTask {
        debug_assert!(self.can_execute());
        self.queue.get_unchecked_mut(self.execution_head)
    }

    /// Move the head of the execution queue into the retire queue.
    ///
    /// **Safety:** The method is only valid when the execution queue is not empty. Additionally,
    /// you should only call this once both result slots and a task cache slot of the pending
    /// task have been filled.
    pub unsafe fn move_to_retire(&mut self) {
        debug_assert!(self.can_execute());
        self.execution_head = (self.execution_head + 1) % LEN;
    }

    /// Obtain the reference to the task that should be retired next.
    ///
    /// **Safety:** If the method is called on an empty retire queue, the result is a valid
    /// reference, but its contents are undefined.
    pub unsafe fn retire_task_reference(&mut self) -> &mut PendingTask {
        debug_assert!(self.can_retire());
        self.queue.get_unchecked_mut(self.retire_head)
    }

    /// Free up the head of the retirement queue.
    ///
    /// **Safety:** The operation is valid only if the retire queue is not empty. Additionally,
    /// retiring a task before it is committed to node storage, task cache and ROB will break
    /// subsequent invariants.
    pub unsafe fn retire(&mut self) {
        debug_assert!(self.can_retire());
        self.retire_head = (self.retire_head + 1) % LEN;
    }

}

/// Similar to `PendingTask`, but does not have a `node_cache_slot`. Also `rob_slot` is replaced
/// with a generic metadata field (which is kept at 4 bytes due to padding).
#[derive(Clone, Default)]
struct StackTask {
    metadata: u32,
    variable: u32,
    left: u64,
    right: u64,
    task_cache_slot: u64,
}

impl StackTask {

    pub fn is_decoded(&self) -> bool {
        self.variable != u32::MAX
    }

}

const SLOT_MASK: u64 = 1 << 63;

struct TaskStack {
    stack: Vec<StackTask>,
    index_after_top: usize, // next element after the stack top
}

impl TaskStack {

    pub fn new(capacity: usize) -> TaskStack {
        TaskStack {
            stack: vec![StackTask::default(); capacity],
            index_after_top: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.index_after_top == 0
    }

    /// Create a new task on the stack, which is either a low or high subtask of the currently
    /// decoded task.
    ///
    /// **Safety:** The function does not check stack overflow, which should not happend if all
    /// other invariants are preserved.
    pub unsafe fn push(&mut self, offset: u32, left: NodeId, right: NodeId) {
        let slot = self.stack.get_unchecked_mut(self.index_after_top);
        // We don't actually have to update the rest of the variables because the will not
        // be read before they are set.
        slot.metadata = offset;
        slot.variable = u32::MAX; // This is used to differentiate decoded tasks.
        slot.left = left.0;
        slot.right = right.0;
        self.index_after_top += 1;
    }

    /// Get a reference to the top task on the stack.
    ///
    /// **Safety:** The method is only valid on a non-empty stack.
    pub unsafe fn peek(&mut self) -> &mut StackTask {
        self.stack.get_unchecked_mut(self.index_after_top - 1)
    }

    /// Pop the current stack top, saving the result of the task into the task that spawned it.
    pub unsafe fn pop_with_node_id(&mut self, result: NodeId) {
        self.pop_with_result(result.0);
    }

    pub unsafe fn pop_with_slot_id(&mut self, result: RobSlot) {
        self.pop_with_result(result.0 | SLOT_MASK);
    }

    unsafe fn pop_with_result(&mut self, result: u64) {
        let offset = self.stack.get_unchecked(self.index_after_top - 1).metadata;
        self.index_after_top -= 1;
        let spawned_by = self.stack.get_unchecked_mut(self.index_after_top - (offset as usize));
        if offset == 2 {
            // Offset 2 is low task, because it means there is still high task on the stack.
            spawned_by.left = result;
        } else {
            // Other offset is interpreted as high task.
            spawned_by.right = result;
        }
    }

}


pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> Bdd {
    let variables = left_bdd.variable_count();
    let mut task_stack = TaskStack::new(2 * (left_bdd.variable_count() as usize) + 2);
    let mut execution_queue = ExecutionRetireQueue::<64>::new();
    let mut reorder_buffer = ReorderBuffer::new(2 * (left_bdd.variable_count() as usize) + 2);

    unsafe {
        // Root task has offset 0 which will cause it to output into itself, which is a bit weird
        // but should be ok.
        task_stack.push(0, left_bdd.root_node(), right_bdd.root_node());
    }

    let mut node_cache = NodeCache::new(left_bdd.node_count());
    let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());

    unsafe {
        while !task_stack.is_empty() || !execution_queue.is_empty() {
            if execution_queue.can_retire() {
                let task = execution_queue.retire_task_reference();
                if task.rob_slot == u32::MAX {
                    // Task was resolved by reduction, move on.
                    execution_queue.retire();
                } else {
                    let mut found = u64::MAX;
                    if !node_cache.contains_at(task.node_cache_slot, task.variable, task.result_low, task.result_high) {
                        if node_cache.has_successor(task.node_cache_slot) {
                            task.node_cache_slot == node_cache.get_successor(task.node_cache_slot);
                        } else {
                            found = node_cache.add_new(task.node_cache_slot, task.variable, task.result_low, task.result_high);
                        }
                    } else {
                        found = task.node_cache_slot;
                    }

                    if found != u64::MAX {
                        let rob_slot = RobSlot(task.rob_slot as u64);
                        reorder_buffer.set_slot_value(rob_slot, NodeId(found));
                        task_cache.write_at(found, task.node_cache_slot, etc);
                    }
                }
            }
            if execution_queue.can_execute() {
                let task = execution_queue.execute_task_reference();
                if task.has_low_result() && task.has_high_result() {
                    let low = NodeId(task.result_low);
                    let high = NodeId(task.result_high);

                    if low == high {
                        let rob_slot = RobSlot(task.rob_slot as u64);
                        reorder_buffer.set_slot_value(rob_slot, low);
                        task_cache.write_at(low, task.node_cache_slot, etc);
                        // This tells the retirement stage that the task is done and can be skipped.
                        task.rob_slot == u32::MAX;
                    } else {
                        task.node_cache_slot = node_cache.find_slot(task.variable, low, high);
                    }
                    execution_queue.move_to_retire();
                } else {
                    if !task.has_low_result() {
                        let rob_slot = RobSlot(task.rob_slot as u64);
                        let value = reorder_buffer.get_slot_value(rob_slot);
                        if !value.is_undefined() {
                            task.result_low = value.0;
                            reorder_buffer.free_slot(rob_slot);
                        }
                    }
                    if !task.has_high_result() {
                        let rob_slot = RobSlot(task.rob_slot as u64);
                        let value = reorder_buffer.get_slot_value(rob_slot);
                        if !value.is_undefined() {
                            task.result_high = value.0;
                            reorder_buffer.free_slot(rob_slot);
                        }
                    }
                }
            }
            if !task_stack.is_empty() {
                let top_task = task_stack.peek();
                if !top_task.is_decoded() {
                    let left = NodeId(top_task.left);
                    let right = NodeId(top_task.right);
                    if left.is_one() || right.is_one() {
                        task_stack.pop_with_node_id(NodeId::ONE);
                    } else if left.is_zero() && right.is_zero() {
                        task_stack.pop_with_node_id(NodeId::ZERO);
                    } else {
                        let (cached_node, cache_slot) = task_cache.read(left, right);
                        if !cached_node.is_undefined() {
                            task_stack.pop_with_node_id(cached_node);
                        } else {
                            // The task is not trivial, nor is it cached. We have to decode it
                            // into two subtasks:
                            let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                            let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                            let decision_variable = min(left_node.variable(), right_node.variable());

                            let (left_low, left_high) = if decision_variable == left_node.variable() {
                                left_node.links()
                            } else {
                                (left, left)
                            };

                            let (right_low, right_high) = if decision_variable == right_node.variable() {
                                right_node.links()
                            } else {
                                (right, right)
                            };

                            top_task.variable = decision_variable.0 as u32;
                            top_task.task_cache_slot = cache_slot;
                            task_stack.push(1, left_high, right_high);
                            task_stack.push(2, left_low, right_low);
                        }
                    }
                }
                let top_task = task_stack.peek();
                if top_task.is_decoded() && !reorder_buffer.is_full() && !execution_queue.is_full() {
                    // Task is already decoded, so both of its sub-tasks should refer either
                    // to ROB slots or exact Node ids. We can move it into execution queue
                    // for further processing, but first we have to find it a slot in the ROB.
                    let rob_slot = reorder_buffer.allocate_slot();
                    execution_queue.enqueue_for_execution(PendingTask {
                        rob_slot: rob_slot.0 as u32,
                        variable: top_task.variable,
                        result_low: top_task.left,
                        result_high: top_task.right,
                        task_cache_slot: top_task.task_cache_slot,
                        node_cache_slot: u64::MAX,
                    });
                    task_stack.pop_with_slot_id(rob_slot);
                }
            }
        }
    }

    let mut nodes = node_cache.nodes;
    let node_count = node_cache.index_after_last;

    for (_, i) in nodes.iter_mut() {
        *i = 0;
    }
    // First two entries are reserved for terminals:
    nodes[0] = ((0, 0), 0);
    nodes[1] = ((1, 1), 1);

    let mut new_index = 2;

    let new_root = NodeId(task_stack.stack[0].right);
    let mut stack = Vec::with_capacity(2 * usize::from(variables));
    unsafe { stack.set_len(stack.capacity()) };
    stack[0] = new_root;
    let mut index_after_last = 1;

    while index_after_last > 0 {
        index_after_last -= 1;
        // Unpack node
        let top = unsafe { *stack.get_unchecked(index_after_last) };
        let node_data = unsafe { nodes.get_unchecked_mut(top.as_index_unchecked()) };
        let (low, high) = (NodeId(node_data.0 .0 & ID_MASK), NodeId(node_data.0 .1));

        // Save index
        node_data.1 = new_index;
        new_index += 1;

        // Push new items on search stack
        if !high.is_terminal() {
            let high_node = unsafe { nodes.get_unchecked_mut(high.as_index_unchecked()) };
            if high_node.1 == 0 {
                unsafe {
                    *stack.get_unchecked_mut(index_after_last) = high;
                    index_after_last += 1;
                }
            }
        }

        if !low.is_terminal() {
            let low_node = unsafe { nodes.get_unchecked_mut(low.as_index_unchecked()) };
            if low_node.1 == 0 {
                unsafe {
                    *stack.get_unchecked_mut(index_after_last) = low;
                    index_after_last += 1;
                }
            }
        }
    }

    let mut new_nodes = Vec::with_capacity(node_count + 1);
    new_nodes.push(BddNode(VariableId::UNDEFINED, NodeId::ZERO, NodeId::ZERO));
    new_nodes.push(BddNode(VariableId::UNDEFINED, NodeId::ONE, NodeId::ONE));
    unsafe { new_nodes.set_len(node_count) };

    for i in 2..node_count {
        let original_node = unsafe { nodes.get_unchecked(i) };
        let variable = ((original_node.0 .0 & VARIABLE_MASK) >> 48) as u16;
        let (low, high) = (
            NodeId(original_node.0 .0 & ID_MASK),
            NodeId(original_node.0 .1),
        );

        let new_low_id = unsafe { NodeId(nodes.get_unchecked(low.as_index_unchecked()).1) };
        let new_high_id = unsafe { NodeId(nodes.get_unchecked(high.as_index_unchecked()).1) };

        let my_new_id = NodeId(original_node.1);

        unsafe {
            *new_nodes.get_unchecked_mut(my_new_id.as_index_unchecked()) =
                BddNode(VariableId(variable), new_low_id, new_high_id);
        }
    }

    Bdd {
        variable_count: variables,
        nodes: new_nodes,
    }
}