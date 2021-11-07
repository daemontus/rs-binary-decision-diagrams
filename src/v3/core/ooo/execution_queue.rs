use crate::v3::core::node_id::NodeId;
use crate::v3::core::ooo::node_cache::NodeCacheSlot;
use crate::v3::core::ooo::reorder_buffer::RobSlot;
use crate::v3::core::ooo::task_cache::TaskCacheSlot;
use crate::v3::core::ooo::task_stack::StackedTask;
use crate::v3::core::packed_bdd_node::PackedBddNode;
use crate::v3::core::variable_id::VariableId;

const ROB_SLOT: u64 = 1 << 63;

#[derive(Clone)]
pub struct PendingTask {
    rob_slot: RobSlot,
    variable: VariableId,
    task: (NodeId, NodeId),
    result: (u64, u64),
    task_cache_slot: TaskCacheSlot,
    node_cache_slot: NodeCacheSlot,
}

impl PendingTask {

    pub fn operands(&self) -> (NodeId, NodeId) {
        self.task
    }

    /// Returns the results of this task as two node ids. This assumes the results are already
    /// computed and is invalid if the results are still in the ROB.
    pub unsafe fn results(&self) -> (NodeId, NodeId) {
        (self.result.0.into(), self.result.1.into())
    }

    pub unsafe fn result_node(&self) -> PackedBddNode {
        PackedBddNode::pack(self.variable, self.result.0.into(), self.result.1.into())
    }

    pub fn has_low_result(&self) -> bool {
        self.result.0 & ROB_SLOT == 0
    }

    pub fn has_high_result(&self) -> bool {
        self.result.1 & ROB_SLOT == 0
    }

    pub fn get_rob(&self) -> RobSlot {
        self.rob_slot
    }

    pub fn get_low_rob(&self) -> RobSlot {
        RobSlot::from((self.result.0 ^ ROB_SLOT) as u32)
    }

    pub fn get_high_rob(&self) -> RobSlot {
        RobSlot::from((self.result.1 ^ ROB_SLOT) as u32)
    }

    pub fn get_low_result(&self) -> NodeId {
        NodeId::from(self.result.0)
    }

    pub fn get_high_result(&self) -> NodeId {
        NodeId::from(self.result.1)
    }

    pub fn set_low_result(&mut self, node: NodeId) {
        self.result.0 = node.into();
    }

    pub fn set_high_result(&mut self, node: NodeId) {
        self.result.1 = node.into();
    }

    pub fn set_node_slot(&mut self, slot: NodeCacheSlot) {
        self.node_cache_slot = slot;
    }

    pub fn get_task_slot(&self) -> TaskCacheSlot {
        self.task_cache_slot
    }
    pub fn get_node_slot(&self) -> NodeCacheSlot {
        self.node_cache_slot
    }

    pub fn get_decision_variable(&self) -> VariableId {
        self.variable
    }

    pub fn mark_as_retired(&mut self) {
        self.rob_slot = RobSlot::UNDEFINED;
    }

    pub fn is_retired(&self) -> bool {
        self.rob_slot == RobSlot::UNDEFINED
    }

}

pub struct ExecutionRetireQueue<const LEN: usize> {
    queue: Vec<PendingTask>,
    retire_head: usize,
    execution_head: usize,
    execution_tail: usize,
}

impl<const LEN: usize> ExecutionRetireQueue<LEN> {

    pub fn new() -> ExecutionRetireQueue<LEN> {
        let mut queue = Vec::with_capacity(LEN);
        unsafe { queue.set_len(LEN); }
        ExecutionRetireQueue {
            queue,
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
    pub unsafe fn enqueue_for_execution(&mut self, rob: RobSlot, task: &StackedTask) {
        debug_assert!(!self.is_full());
        let slot = unsafe { self.queue.get_unchecked_mut(self.execution_tail) };
        *slot = PendingTask {
            rob_slot: rob,
            variable: task.get_decision_variable(),
            task: task.operands(),
            result: task.get_raw_results(),
            task_cache_slot: task.get_task_slot(),
            node_cache_slot: NodeCacheSlot::UNDEFINED,
        };
        self.execution_tail = (self.execution_tail + 1) % LEN
    }

    /// Obtain the reference to the task that should be executed next.
    ///
    /// **Safety:** If the method is called on an empty queue, the resulting reference is valid,
    /// but its contents are undefined.
    pub unsafe fn execute_task_reference(&mut self) -> &mut PendingTask {
        debug_assert!(self.can_execute());
        unsafe { self.queue.get_unchecked_mut(self.execution_head) }
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
        unsafe { self.queue.get_unchecked_mut(self.retire_head) }
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
