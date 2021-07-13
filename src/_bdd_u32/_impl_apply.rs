use crate::{Bdd, Pointer, Variable};
use crate::_bdd_u32::{CompleteTaskQueue, PartialTaskCache, PartialNodeCache};
use std::cmp::{max, min};

pub fn and_not(left: &Bdd, right: &Bdd) -> Bdd {
    let variable_count = max(left.variable_count(), right.variable_count());
    let larger_size = max(left.node_count(), right.node_count());

    // First, generate all the tasks
    let mut task_queue = CompleteTaskQueue::new(variable_count, 2 * larger_size);
    let mut task_cache = PartialTaskCache::new(2 * larger_size);
    let mut task_stack = Vec::<((Pointer, Pointer), usize)>::with_capacity(2 * (variable_count as usize));
    let mut node_cache = PartialNodeCache::new(2 * larger_size);

    let left_root = left.root_pointer();
    let right_root = right.root_pointer();
    let root_variable = min(left.var_of(left_root), right.var_of(right_root));
    let root_index = task_queue.reserve_task(root_variable);
    task_stack.push(((left_root, right_root), root_index));

    let mut is_not_empty = false;
    while let Some(task) = task_stack.pop() {
        expand_task(left, right, task, &mut task_queue, &mut task_cache, &mut task_stack, &mut is_not_empty);
    }

    if !is_not_empty {
        return Bdd::new_false();
    }

    //println!("Generated {} tasks for {} and {}.", task_queue.tasks.len(), left.node_count(), right.node_count());

    let mut result = Bdd::new_true_with_variables(variable_count);

    for i_v in (root_variable.0 .. variable_count).rev() {
        let variable = Variable(i_v);
        let mut to_process = task_queue.variable_iteration(variable);
        while to_process != 0 {
            let (low_task, high_task) = task_queue.tasks[to_process].dependencies;
            let low_result = task_queue.tasks[low_task].result;
            let high_result = task_queue.tasks[high_task].result;

            if low_result == high_result {
                task_queue.tasks[to_process].result = low_result;
            } else {
                let saved_pointer = node_cache.read(variable, low_result | high_result, &result);
                if saved_pointer.is_undef() {
                    let pointer = result.create_node(variable, low_result, high_result);
                    task_queue.tasks[to_process].result = pointer;
                    node_cache.write(variable, low_result | high_result, pointer);
                } else {
                    task_queue.tasks[to_process].result = saved_pointer;
                }
            }

            to_process = task_queue.tasks[to_process].next_task;
        }
    }

    result
}

#[inline]
fn expand_task(
    left: &Bdd,
    right: &Bdd,
    task: ((Pointer, Pointer), usize),
    queue: &mut CompleteTaskQueue,
    cache: &mut PartialTaskCache,
    stack: &mut Vec<((Pointer, Pointer), usize)>,
    is_not_empty: &mut bool,
) {
    let ((l, r), queue_index) = task;
    let (l_var, r_var) = (left.var_of(l), right.var_of(r));
    let decision_variable = min(l_var, r_var);

    let (l_low, l_high) = if l_var != decision_variable {
        (l, l)
    } else {
        left.pointers_of(l).unpack()
    };

    let (r_low, r_high) = if r_var != decision_variable {
        (r, r)
    } else {
        right.pointers_of(r).unpack()
    };

    let cached_low = cache.read(l_low, r_low);
    let cached_high = cache.read(l_high, r_high);

    let low_queue_index = if l_low.is_zero() || r_low.is_one() {
        0
    } else if l_low.is_one() && r_low.is_zero() {
        1
    } else if cached_low != usize::MAX {
        cached_low
    } else {
        let var = min(left.var_of(l_low), right.var_of(r_low));
        let index = queue.reserve_task(var);
        stack.push(((l_low, r_low), index));
        cache.write(l_low, r_low, index);
        index
    };

    let high_queue_index = if l_high.is_zero() || r_high.is_one() {
        0
    } else if l_high.is_one() && r_high.is_zero() {
        1
    } else if cached_high != usize::MAX {
        cached_high
    } else {
        let var = min(left.var_of(l_high), right.var_of(r_high));
        let index = queue.reserve_task(var);
        stack.push(((l_high, r_high), index));
        cache.write(l_high, r_high, index);
        index
    };


    *is_not_empty = *is_not_empty || low_queue_index == 1 || high_queue_index == 1;
    queue.set_dependencies(queue_index, (low_queue_index, high_queue_index));
}