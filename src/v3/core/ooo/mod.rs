use std::cmp::min;
use crate::v3::core::bdd::Bdd;
use crate::v3::core::node_id::NodeId;
use crate::v3::core::ooo::execution_queue::ExecutionRetireQueue;
use crate::v3::core::ooo::node_cache::NodeCache;
use crate::v3::core::ooo::reorder_buffer::ReorderBuffer;
use crate::v3::core::ooo::task_cache::TaskCache;
use crate::v3::core::ooo::task_stack::TaskStack;

pub mod task_cache;
pub mod node_cache;
pub mod task_stack;
pub mod reorder_buffer;
pub mod execution_queue;

pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> Bdd {
    let mut queue = ExecutionRetireQueue::<64>::new();
    let mut rob = ReorderBuffer::new(left_bdd.get_height() + right_bdd.get_height());
    let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());
    let mut node_cache = NodeCache::new(left_bdd.node_count(), 2 * left_bdd.node_count());
    let mut stack = TaskStack::new(left_bdd.get_height(), right_bdd.get_height());
    unsafe {
        stack.push_new(0, (left_bdd.get_root_id(), right_bdd.get_root_id()));

        while !stack.is_empty() {
            if queue.can_retire() {
                let task = queue.retire_task_reference();
                if task.is_retired() { // The task was retired during the execute step.
                    queue.retire()
                } else {
                    match node_cache.ensure(&task.result_node()) {
                        Ok(id) => {
                            rob.set_slot_value(task.get_rob(), id);
                            task_cache.write_unchecked(task.operands(), id, task.get_task_slot());
                            queue.retire();
                        }
                        Err(slot) => {
                            task.set_node_slot(slot);
                        }
                    }
                }
            }
            if queue.can_execute() {
                let task = queue.execute_task_reference();
                if task.has_low_result() && task.has_high_result() {
                    let low_result = task.get_low_result();
                    let high_result = task.get_high_result();

                    if low_result == high_result {
                        // The node exists, we just need to mark it as a result of this task
                        // and it can be immediately retired (will be skipped in retire queue).
                        rob.set_slot_value(task.get_rob(), low_result);
                        task_cache.write_unchecked(task.operands(), low_result, task.get_task_slot());
                        task.mark_as_retired();
                    } else {
                        // We actually need to query the node cache to check if this exists or not.
                        match node_cache.ensure(&task.result_node()) {
                            Ok(id) => {
                                // Node is already cached, just update result.
                                rob.set_slot_value(task.get_rob(), id);
                                task_cache.write_unchecked(task.operands(), id, task.get_task_slot());
                                task.mark_as_retired();
                            }
                            Err(slot) => {
                                // Node was not found here, try later.
                                task.set_node_slot(slot);
                            }
                        }
                    }
                    // Regardless of what happened, the task is moving into retire.
                    queue.move_to_retire();
                } else {
                    if !task.has_low_result() {
                        let slot = task.get_low_rob();
                        let result = rob.get_slot_value(slot);
                        if !result.is_undefined() {
                            rob.free_slot(slot);
                            task.set_low_result(result);
                        }
                    }
                    if !task.has_high_result() {
                        let slot = task.get_high_rob();
                        let result = rob.get_slot_value(slot);
                        if !result.is_undefined() {
                            rob.free_slot(slot);
                            task.set_high_result(result);
                        }
                    }
                }
            }
            let task = stack.get_top_mut();
            if task.is_decoded() {
                // The task should have results declared and can be moved to the execution queue.
                if !rob.is_full() && !queue.is_full() {
                    let slot = rob.allocate_slot();
                    queue.enqueue_for_execution(slot, task);
                    stack.pop_with_slot_id(slot);
                } else {
                    panic!("Frontend stall.")
                }
            } else {
                // The task is newly created and must be decoded.
                let (left, right) = task.operands();
                if left.is_one() || right.is_one() {
                    stack.pop_with_node_id(NodeId::ONE);
                } else if left.is_zero() && right.is_zero() {
                    stack.pop_with_node_id(NodeId::ZERO);
                } else {
                    let task_slot = task_cache.find_slot((left, right));
                    let cached_node = task_cache.read_unchecked((left, right), task_slot);
                    if !cached_node.is_undefined() {
                        stack.pop_with_node_id(cached_node);
                    } else {
                        // Actually decode the task into two sub-tasks that will be pushed on
                        // the stack. Also, update task with computed data.
                        let left_node = left_bdd.get_node_unchecked(left);
                        let right_node = right_bdd.get_node_unchecked(right);
                        let (left_var, left_low, left_high) = left_node.unpack();
                        let (right_var, right_low, right_high) = right_node.unpack();

                        let decision_variable = min(left_var, right_var);

                        let (left_low, left_high) = if decision_variable == left_var {
                            (left_low, left_high)
                        } else {
                            (left, left)
                        };

                        let (right_low, right_high) = if decision_variable == right_var {
                            (right_low, right_high)
                        } else {
                            (right, right)
                        };

                        task.set_decoded();
                        task.set_task_slot(task_slot);
                        task.set_decision_variable(decision_variable);

                        stack.push_new(1, (left_high, right_high));
                        stack.push_new(2, (left_low, right_low));
                    }
                }
            }
        }
    }

    unimplemented!()
}