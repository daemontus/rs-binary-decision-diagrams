use crate::v2::{Bdd, NodeId, BddNode};
use std::convert::TryFrom;
use std::cmp::{max, min};
use crate::v2::_impl_::bdd::binary_operations::u48::partial_node_cache::NodeCache;
use crate::v2::_impl_::bdd::binary_operations::u32::partial_task_cache::TaskCache;
use crate::v2::_impl_::bdd::binary_operations::u32::coupled_dfs_stack::Stack;

/// Implementation details of the `PointerPair` struct.
mod pointer_pair;

/// A `u32` optimized version of the task stack.
mod coupled_dfs_stack;

/// A `u32` optimized version of the task cache.
mod partial_task_cache;

/// **(internal)** Packs two 32-bit `NodeId` pointers into a single integer, or a single
/// 48-bit one (a result pointer). We mainly keep this representation around and public(-ish)
/// so that it can be used by the stack and hash table and we avoid a conversion.
#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) struct PointerPair(u64);

/// Left `Bdd` can be anything that does not overflow `u32`.
const MAX_LEFT_SIZE: u64 = u32::MAX as u64;
/// Right `Bdd` cannot have the highest bit set.
const MAX_RIGHT_SIZE: u64 = MAX_LEFT_SIZE ^ (1 << 31);

impl Bdd {

    pub fn _u32_and_not(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() < usize::try_from(MAX_LEFT_SIZE).unwrap());
        debug_assert!(other.node_count() < usize::try_from(MAX_RIGHT_SIZE).unwrap());
        let left_bdd = self;
        let right_bdd = other;

        let variables = max(left_bdd.variable_count(), right_bdd.variable_count());

        let mut is_not_false = false;
        let mut node_cache = NodeCache::new(left_bdd.node_count());
        let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());
        let mut stack = Stack::new(variables);
        unsafe {
            let root = PointerPair::pack(left_bdd.root_node(), right_bdd.root_node());
            stack.push_task_unchecked(root);
        }

        // The calls to stack operations are safe due to the order in which we perform the Bdd search.
        loop {
            // If the top is a result, go straight to finishing a task. If not, first expand,
            // but if the result of the expansion is a finished task, then also finish a task.
            let mut finish_task = stack.has_result();

            if !finish_task {
                // Expand current top task.
                let tasks = unsafe { stack.peek_as_task_unchecked() };
                let (left, right) = tasks.unpack();

                if left.is_zero() || right.is_one() {
                    finish_task = finish_task || unsafe { stack.save_result_unchecked(NodeId::ZERO) };
                } else if left.is_one() && right.is_zero() {
                    is_not_false = true;
                    finish_task = finish_task || unsafe { stack.save_result_unchecked(NodeId::ONE) };
                } else {
                    let cached_node = task_cache.read(tasks);
                    if !cached_node.is_undefined() {
                        finish_task =
                            finish_task || unsafe { stack.save_result_unchecked(cached_node) };
                    } else {
                        let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                        let right_node = unsafe { right_bdd.get_node_unchecked(right) };
                        let (left_var, left_low, left_high) = left_node.unpack();
                        let (right_var, right_low, right_high) = right_node.unpack();
                        left_bdd.prefetch(left_low);
                        right_bdd.prefetch(right_low);

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

                        let low_tasks = PointerPair::pack(left_low, right_low);
                        let high_tasks = PointerPair::pack(left_high, right_high);

                        task_cache.prefetch(high_tasks);

                        // When completed, the order of tasks will be swapped (high on top).
                        unsafe {
                            stack.push_task_unchecked(high_tasks);
                            stack.push_task_unchecked(low_tasks);
                        }
                    }
                }
            }

            if finish_task {
                // Finish current top task.
                let (low, high) = unsafe { stack.pop_results_unchecked() };
                let task = unsafe { stack.peek_as_task_unchecked() };

                if high == low {
                    task_cache.write(task, low);
                    unsafe { stack.save_result_unchecked(low) };
                } else {
                    let (left, right) = task.unpack();
                    let (left_var, right_var) =
                        (left_bdd.get_variable(left), right_bdd.get_variable(right));
                    let decision_variable = min(left_var, right_var);

                    let node = BddNode::pack(decision_variable, low, high);
                    let result_id = node_cache.ensure(node);
                    task_cache.write(task, result_id);
                    unsafe { stack.save_result_unchecked(result_id) };
                }
            }

            if stack.has_last_entry() {
                break; // The last entry is the result to the first task.
            }
        }

        if is_not_false {
            let mut result = node_cache.export();
            result.update_variable_count(variables);
            result
        } else {
            Bdd::new_false()
        }
    }

}