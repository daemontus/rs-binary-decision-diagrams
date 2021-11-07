use std::ops::Not;
use crate::v3::core::node_id::NodeId;
use crate::v3::core::ooo::reorder_buffer::RobSlot;
use crate::v3::core::ooo::task_cache::TaskCacheSlot;
use crate::v3::core::variable_id::VariableId;

const NOT_DECODED: u32 = 1 << 31;
const ROB_SLOT: u64 = 1 << 63;

pub struct StackedTask {
    // How many slots "below" this one is the spawning task.
    offset: u32,
    // Serialised decision variable of this task.
    variable: VariableId,
    // Serialised left/right BDD node ids.
    task: (NodeId, NodeId),
    // Either a valid NodeId, or a SlotId in the reorder buffer
    // where the result will be obtained.
    results: (u64, u64),
    // A place to save the
    task_cache_slot: TaskCacheSlot,
}

impl StackedTask {

    pub fn is_decoded(&self) -> bool {
        self.offset & NOT_DECODED == 0
    }

    pub fn set_decoded(&mut self) {
        self.offset = self.offset & NOT_DECODED.not();
    }

    pub fn operands(&self) -> (NodeId, NodeId) {
        self.task
    }

    pub fn set_task_slot(&mut self, slot: TaskCacheSlot) {
        self.task_cache_slot = slot;
    }

    pub fn get_task_slot(&self) -> TaskCacheSlot {
        self.task_cache_slot
    }

    pub fn set_decision_variable(&mut self, variable: VariableId) {
        self.variable = variable;
    }

    pub fn get_decision_variable(&self) -> VariableId {
        self.variable
    }

    pub fn get_raw_results(&self) -> (u64, u64) {
        self.results
    }
}

pub struct TaskStack {
    index_after_last: usize,
    items: Vec<StackedTask>
}

impl TaskStack {

    pub fn new(height_left: usize, height_right: usize) -> TaskStack {
        let mut items = Vec::with_capacity(height_left + height_right);
        unsafe {
            items.set_len(height_left + height_right);
        }
        TaskStack {
            index_after_last: 0,
            items,
        }
    }

    pub unsafe fn push_new(&mut self, offset: u32, task: (NodeId, NodeId)) {
        let slot = unsafe { self.items.get_unchecked_mut(self.index_after_last) };
        self.index_after_last += 1;
        slot.task = task;
        slot.offset = offset | NOT_DECODED;
        // The rest of the slot data is left uninitialized.
    }

    pub fn is_empty(&self) -> bool {
        self.index_after_last == 0
    }

    pub unsafe fn get_top_mut(&mut self) -> &mut StackedTask {
        unsafe { self.items.get_unchecked_mut(self.index_after_last - 1) }
    }

    pub unsafe fn pop_with_node_id(&mut self, result: NodeId) {
        unsafe { self.pop_with_result(result.into()); }
    }

    pub fn len(&self) -> usize {
        self.index_after_last
    }

    pub unsafe fn pop_with_slot_id(&mut self, result: RobSlot) {
        unsafe { self.pop_with_result(u64::from(u32::from(result)) | ROB_SLOT); }
    }

    unsafe fn pop_with_result(&mut self, result: u64) {
        self.index_after_last -= 1;
        let top = unsafe { self.items.get_unchecked(self.index_after_last) };
        let offset = (top.offset & NOT_DECODED.not()) as usize;   // can be only 0/1/2.
        let output = unsafe { self.items.get_unchecked_mut(self.index_after_last - offset) };
        // offset = 1 is "high", offset = 2 is "low"
        // Also, special case offset = 0 (root task) is also low.
        if offset == 1 {
            output.results.1 = result;
        } else {
            output.results.0 = result;
        }
    }



}