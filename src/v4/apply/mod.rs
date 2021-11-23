mod task_cache;
mod unsafe_stack;
mod node_cache;

use super::core::{Bdd, Variable, NodeIndex, Node};
use task_cache::{TaskCache, TaskCacheSlot};
use node_cache::NodeCache;
use unsafe_stack::UnsafeStack;
use crate::IntoIndex;

#[derive(Copy, Clone, Eq, PartialEq)]
struct ApplyTask {
    offset: u8,
    variable: Variable,
    task: (NodeIndex, NodeIndex),
    results: [NodeIndex; 2],
    task_cache_slot: TaskCacheSlot,
}

impl ApplyTask {

    pub fn new(offset: u8, task: (NodeIndex, NodeIndex)) -> ApplyTask {
        ApplyTask {
            offset: offset << 1,
            task,
            variable: Variable::UNDEFINED,
            results: [NodeIndex::UNDEFINED, NodeIndex::UNDEFINED],
            task_cache_slot: TaskCacheSlot::UNDEFINED,
        }
    }

    #[inline]
    pub fn get_offset(&self) -> u8 {
        self.offset >> 1
    }

    #[inline]
    pub fn is_not_decoded(&self) -> bool {
        self.offset & 1 == 0
    }

    #[inline]
    pub fn mark_as_decoded(&mut self) {
        self.offset = self.offset | 1;
    }
}

pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> (usize, usize) {
    let height_limit = left_bdd.get_height() + right_bdd.get_height();
    let mut task_cache = TaskCache::new(left_bdd.get_size());
    let mut node_cache = NodeCache::new(3 * left_bdd.get_size());
    let mut task_count = 0;

    // There are up to height_limit expanded tasks and every task has up to one extra non-expanded
    // child.
    let mut stack = UnsafeStack::new(2 * height_limit.into_index());
    stack.push(ApplyTask::new(0, (left_bdd.get_root_index(), right_bdd.get_root_index())));

    while !stack.is_empty() {
        let top = stack.peek();
        let top_offset = top.get_offset(); // Save for later...

        let mut result = NodeIndex::UNDEFINED;
        // We could also try using top.variable, but this version seems to be faster
        // due to easier branch prediction.
        if top.is_not_decoded() {
            top.mark_as_decoded();

            let (left, right) = top.task;
            if left.is_one() || right.is_one() {
                result = NodeIndex::ONE;
            } else if left.is_zero() && right.is_zero() {
                result = NodeIndex::ZERO;
            } else {
                let (cached, slot) = task_cache.read(top.task);
                if !cached.is_undefined() {
                    result = cached;
                } else {
                    top.task_cache_slot = slot;
                    task_count += 1;
                    // Actually expand this task into sub-tasks.

                    let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                    let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                    let (l_var, l_low, l_high) = left_node.unpack();
                    let (r_var, r_low, r_high) = right_node.unpack();

                    // This explicit "switch" is slightly faster. Not sure exactly why, but
                    // it is probably easier to branch predict.
                    if l_var == r_var {
                        top.variable = l_var;
                        stack.push(ApplyTask::new(1, (l_high, r_high)));
                        stack.push(ApplyTask::new(2, (l_low, r_low)));
                    } else if l_var < r_var {
                        top.variable = l_var;
                        stack.push(ApplyTask::new(1, (l_high, right)));
                        stack.push(ApplyTask::new(2, (l_low, right)));
                    } else {
                        top.variable = r_var;
                        stack.push(ApplyTask::new(1, (left, r_high)));
                        stack.push(ApplyTask::new(2, (left, r_low)));
                    }
                }
            }
        } else {
            // Task is decoded, we have to create a new node for it.
            let (result_low, result_high) = (top.results[1], top.results[0]);
            if result_low == result_high {
                task_cache.write(top.task_cache_slot, top.task, result_low);
                result = result_low;
            } else {
                let node = Node::pack(top.variable, result_low, result_high);

                let mut cached = node_cache.ensure(&node);
                while let Err(slot) = cached {
                    cached = node_cache.ensure_at(&node, slot);
                }
                result = cached.unwrap();
                task_cache.write(top.task_cache_slot, top.task, result);
            }
        }

        if !result.is_undefined() {
            stack.pop();
            if !stack.is_empty() {
                let top_offset: usize = top_offset.into();
                // Offset one is the top task, offset two is the one beneath that.
                let parent = stack.peek_at(top_offset);
                // high = 1, low = 2, so they will be saved in reverse order.
                let slot = unsafe {
                    parent.results.get_unchecked_mut(top_offset - 1)
                };
                *slot = result;
            }
        }
    }

    (node_cache.len(), task_count)
}