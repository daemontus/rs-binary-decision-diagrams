use crate::_bdd_u32::PartialNodeCache;
use crate::{Bdd, Pointer, PointerPair, Variable, SEED64};
use std::cmp::{max, min};
use std::num::NonZeroU64;
use std::ops::{Not, Rem, Shl, Shr};
use std::process::exit;

pub struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<u64>,
    values: Vec<Pointer>,
}

impl TaskCache {
    pub fn new(capacity: usize) -> TaskCache {
        TaskCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            keys: vec![u64::MAX; capacity],
            values: vec![Pointer::undef(); capacity],
        }
    }

    #[inline]
    pub fn read(&self, x: Pointer, y: Pointer) -> Pointer {
        let packed = x | y;
        let index = self.hash(packed);
        unsafe {
            if *self.keys.get_unchecked(index) == packed.0 {
                *self.values.get_unchecked(index)
            } else {
                Pointer::undef()
            }
        }
    }

    #[inline]
    pub fn write(&mut self, x: Pointer, y: Pointer, result: Pointer) {
        let packed = x | y;
        let index = self.hash(packed);
        unsafe {
            *self.keys.get_unchecked_mut(index) = packed.0;
            *self.values.get_unchecked_mut(index) = result;
        }
    }

    #[inline]
    pub fn hash(&self, pointers: PointerPair) -> usize {
        pointers.0.wrapping_mul(SEED64).rem(self.capacity) as usize
    }
}

const CHILD_BIT_MASK: u64 = 1u64 << 32;
const EXPANDED_BIT_MASK: u64 = 1u64 << 33;
const DONE_BIT_MASK: u64 = 1u64 << 34;

#[derive(Clone)]
struct Task {
    input: (Pointer, Pointer),
    output: (Pointer, Pointer),
    parent: u64,
}

// Stack entries work in two "modes".
// We use the two most significant bits in the u64 "parent" to determine
// which task, and whether we are the low/high child (bit one).
// Furthermore, the second bit stores whether this task has been expanded or not.
pub struct UnrolledStack {
    index_after_top: usize,
    items: Vec<Task>,
}

impl UnrolledStack {
    pub fn new(capacity: usize) -> UnrolledStack {
        UnrolledStack {
            index_after_top: 0,
            items: vec![
                Task {
                    input: (Pointer::zero(), Pointer::zero()),
                    output: (Pointer::zero(), Pointer::zero()),
                    parent: 0,
                };
                capacity
            ],
        }
    }

    /// Reserves space for 16 more elements on the stack
    #[inline]
    pub fn reserve_capacity_16(&mut self) {
        if self.items.len() < self.index_after_top + 16 {
            self.items.reserve(16);
            // We don't have to initialize the pointers because we only care what's before top.
            unsafe { self.items.set_len(self.items.capacity()) };
        }
    }

    #[inline]
    pub fn push_new_task(
        &mut self,
        input: (Pointer, Pointer),
        mut parent: usize,
        is_high_child: bool,
    ) {
        let mut parent = parent as u64;
        if is_high_child {
            parent = parent | CHILD_BIT_MASK;
        }
        unsafe {
            *self.items.get_unchecked_mut(self.index_after_top) = Task {
                input,
                output: (Pointer::undef(), Pointer::undef()),
                parent,
            }
        };
        self.index_after_top += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.index_after_top
    }

    #[inline]
    pub fn pop(&mut self) {
        self.index_after_top -= 1;
    }

    #[inline]
    pub fn is_expanded(&self, task_id: usize) -> bool {
        unsafe { self.items.get_unchecked(task_id).parent & EXPANDED_BIT_MASK != 0 }
    }

    #[inline]
    pub fn mark_expanded(&mut self, task_id: usize) {
        unsafe {
            let cell = self.items.get_unchecked_mut(task_id);
            cell.parent = cell.parent | EXPANDED_BIT_MASK;
        }
    }

    #[inline]
    pub fn is_done(&self, task_id: usize) -> bool {
        unsafe { self.items.get_unchecked(task_id).parent & DONE_BIT_MASK != 0 }
    }

    #[inline]
    pub fn get_input(&self, task_id: usize) -> (Pointer, Pointer) {
        unsafe { self.items.get_unchecked(task_id).input }
    }

    #[inline]
    pub fn get_output(&self, task_id: usize) -> (Pointer, Pointer) {
        unsafe { self.items.get_unchecked(task_id).output }
    }

    #[inline]
    pub fn save_result_and_mark_done(&mut self, task_id: usize, result: Pointer) {
        unsafe {
            let my_cell = self.items.get_unchecked_mut(task_id);
            let parent_pointer = my_cell.parent;
            let parent_index = (parent_pointer as u32) as usize;
            my_cell.parent = DONE_BIT_MASK;
            let parent_cell = self.items.get_unchecked_mut(parent_index);
            if parent_pointer & CHILD_BIT_MASK != 0 {
                parent_cell.output.1 = result;
            } else {
                parent_cell.output.0 = result;
            }
        }
    }
}

pub fn gen_tasks(
    left: &Bdd,
    right: &Bdd,
    task_cache: &mut TaskCache,
    stack: &mut UnrolledStack,
) -> usize {
    let variable_count = max(left.variable_count(), right.variable_count());
    let larger_size = max(left.node_count(), right.node_count());

    let mut result = Bdd::new_true_with_variables(variable_count);
    let node_cache = &mut PartialNodeCache::new(2 * larger_size);
    let task_cache = &mut TaskCache::new(2 * larger_size);
    let stack = &mut UnrolledStack::new(2 * usize::from(variable_count)); //Vec::with_capacity(2 * usize::from(variable_count));
    stack.push_new_task((left.root_pointer(), right.root_pointer()), 0, false);
    //stack.reserved_push(left.root_pointer(), right.root_pointer(), 2);

    let mut iteration_count = 0;
    /*while let Some(task) = stack.pop() {
        iteration_count += 1;
        expand_task(task, &mut task_cache, &mut stack, left, right);
    }*/
    loop {
        stack.reserve_capacity_16();
        if stack.len() > 0 {
            iteration_count += 1;
            if step(
                stack.len() - 1,
                task_cache,
                node_cache,
                stack,
                &mut result,
                left,
                right,
            ) {
                stack.pop();
            }
        } else {
            break;
        }
    }

    result.node_count()
}

#[inline]
fn step(
    task_id: usize,
    task_cache: &mut TaskCache,
    node_cache: &mut PartialNodeCache,
    stack: &mut UnrolledStack,
    result: &mut Bdd,
    left_bdd: &Bdd,
    right_bdd: &Bdd,
) -> bool {
    if stack.is_done(task_id) {
        // Nothing to be done for this task, just pop if possible.
        true
    } else if stack.is_expanded(task_id) {
        // Task is expanded and so we need to check if its child tasks have completed.
        let (low, high) = stack.get_output(task_id);
        if task_id == 48 && low.is_undef() && !high.is_undef() {
            exit(1);
        }
        // A task can be popped if both pointers are done.
        if low.is_undef() || high.is_undef() {
            false
        } else {
            let (left, right) = stack.get_input(task_id);
            if low == high {
                task_cache.write(left, right, low);
                stack.save_result_and_mark_done(task_id, low);
            } else {
                let (left_var, right_var) = (left_bdd.var_of(left), right_bdd.var_of(right));
                let decision_variable = min(left_var, right_var);

                let mut result_pointer = node_cache.read(decision_variable, (low | high), &result);
                if result_pointer.is_undef() {
                    result_pointer = result.create_node(decision_variable, low, high);
                }

                task_cache.write(left, right, result_pointer);
                stack.save_result_and_mark_done(task_id, result_pointer);
            }
            true
        }
    } else {
        // Try to expand task
        let (left, right) = stack.get_input(task_id);

        if left.is_zero() || right.is_one() {
            // Finish task as zero
            stack.save_result_and_mark_done(task_id, Pointer::zero());
            true
        } else if left.is_one() && right.is_zero() {
            // Finish task as one
            stack.save_result_and_mark_done(task_id, Pointer::one());
            true
        } else {
            let cached_result = task_cache.read(left, right);
            if !cached_result.is_undef() {
                // Someone else finished this for us.
                stack.save_result_and_mark_done(task_id, cached_result);
                true
            } else {
                let (left_var, right_var) = (left_bdd.var_of(left), right_bdd.var_of(right));
                let decision_variable = min(left_var, right_var);

                let (left_low, left_high) = if left_var != decision_variable {
                    (left, left)
                } else {
                    left_bdd.pointers_of(left).unpack()
                };

                let (right_low, right_high) = if right_var != decision_variable {
                    (right, right)
                } else {
                    right_bdd.pointers_of(right).unpack()
                };

                stack.mark_expanded(task_id);
                stack.push_new_task((left_low, right_low), task_id, false);
                stack.push_new_task((left_high, right_high), task_id, true);

                /*unsafe {
                    let hash_low = task_cache.hash(left_low | right_low);
                    let hash_high = task_cache.hash(left_high | right_high);
                    let low_ref: *const u64 = task_cache.keys.get_unchecked(hash_low);
                    let high_ref: *const u64 = task_cache.keys.get_unchecked(hash_high);
                    std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
                    std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

                    let low_ref: *const Pointer = task_cache.values.get_unchecked(hash_low);
                    let high_ref: *const Pointer = task_cache.values.get_unchecked(hash_high);
                    std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
                    std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

                    let pointer: *const u64 = &left_bdd.node_pointers.get_unchecked(left_low.0 as usize).0;
                    std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
                    let pointer: *const u64 = &left_bdd.node_pointers.get_unchecked(left_high.0 as usize).0;
                    std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
                    let pointer: *const u64 = &right_bdd.node_pointers.get_unchecked(right_low.0 as usize).0;
                    std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
                    let pointer: *const u64 = &right_bdd.node_pointers.get_unchecked(right_high.0 as usize).0;
                    std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
                }*/

                false
            }
        }
    }
}
/*
#[inline]
fn expand_task(
    task: (Pointer, Pointer, u64),
    task_cache: &mut TaskCache,
    stack: &mut UnrolledStack,
    left: &Bdd,
    right: &Bdd,
) {
    let (l, r, parent_id) = task;

    // First, check if the task can be resolved using "terminal" rules.
    if l.is_zero() || r.is_one() || (l.is_one() && r.is_zero()) {
        return;
    }

    // Second, check if the task is saved in the cache. If yes, it is already expanded.
    if !task_cache.read(l, r).is_undef() {
        return;
    }

    // If none of it holds, process the task now:

    task_cache.write(l, r, Pointer::one());
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


    stack.reserved_push(l_low, r_low, 0);
    stack.reserved_push(l_high, r_high, 1);
/*
    unsafe {
        let hash_low = task_cache.hash(l_low | r_low);
        let hash_high = task_cache.hash(l_high | r_high);
        let low_ref: *const u64 = task_cache.keys.get_unchecked(hash_low);
        let high_ref: *const u64 = task_cache.keys.get_unchecked(hash_high);
        std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
        std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

        let low_ref: *const Pointer = task_cache.values.get_unchecked(hash_low);
        let high_ref: *const Pointer = task_cache.values.get_unchecked(hash_high);
        std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
        std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

        let pointer: *const u64 = &left.node_pointers.get_unchecked(l_low.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &left.node_pointers.get_unchecked(l_high.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &right.node_pointers.get_unchecked(r_low.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &right.node_pointers.get_unchecked(r_high.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
    }
 */
}
 */
