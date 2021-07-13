use crate::{Bdd, Pointer, PointerPair, SEED64, Variable};
use std::cmp::{max, min};
use std::num::NonZeroU64;
use std::ops::{Shl, Rem, Not, Shr};
use crate::_bdd_u32::PartialNodeCache;
use std::process::exit;

pub struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<u64>,
    values: Vec<Pointer>,
}

impl TaskCache {

    pub fn new(mut capacity: usize) -> TaskCache {
        TaskCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            keys: vec![0; capacity],
            values: vec![Pointer::zero(); capacity],
        }
    }

    pub fn clear(&mut self) {
        for i in self.keys.iter_mut() {
            *i = u64::MAX;
        }
        /*unsafe {
            let len = self.values.len() / 2;
            let pointer: *mut Pointer = self.values.get_unchecked_mut(0);
            let mut mem: *mut u64 = pointer as (*mut u64);
            for i in 0..len {
                *mem.offset(i as isize) = u64::MAX;
            }
        }*/
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

// Stack entries work in two "modes".
// We use the two most significant bits in the u64 "parent" to determine
// which task, and whether we are the low/high child (bit one).
// Furthermore, the second bit stores whether this task has been expanded or not.
pub struct UnrolledStack {
    index_after_top: usize,
    items: Vec<u64>,
}

impl UnrolledStack {

    pub fn new(capacity: usize) -> UnrolledStack {
        let mut x = UnrolledStack {
            index_after_top: 1,
            items: vec![0; capacity],
        };
        // Put one dummy item at the beginning so that it is also safe to access the "-1" position.
        x.items[0] = (Pointer::zero() | Pointer::undef()).0;
        x
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
    pub fn len(&self) -> usize {
        self.index_after_top - 1
    }

    #[inline]
    pub fn push(&mut self, task: PointerPair) {
        unsafe { *self.items.get_unchecked_mut(self.index_after_top) = task.0; }
        self.index_after_top += 1;
    }

    #[inline]
    pub fn is_result(&self, offset: usize) -> bool {
        unsafe {
            let cell = *self.items.get_unchecked(self.index_after_top - 1 - offset);
            (cell as u32) == u32::MAX
        }
    }

    #[inline]
    pub fn pop_result(&mut self) -> Pointer {
        unsafe {
            self.index_after_top -= 1;
            let cell = *self.items.get_unchecked(self.index_after_top);
            Pointer(cell.shr(32) as u32)
        }
    }

    #[inline]
    pub fn peek_task(&self) -> (Pointer, Pointer) {
        unsafe {
            let cell = *self.items.get_unchecked(self.index_after_top - 1);
            PointerPair(cell).unpack()
        }
    }

    #[inline]
    pub fn swap_top(&mut self) {
        self.items.swap(self.index_after_top - 1, self.index_after_top - 2);
        /*unsafe {
            let top_cell = self.items.get_unchecked_mut(self.index_after_top - 1);
            let cell_below = self.items.get_unchecked_mut(self.index_after_top - 2);
            let tmp = *top_cell;
            *top_cell = *cell_below;
            *cell_below = tmp;
        }*/
    }

    /// Marks the top task as result and swaps it with the previous task if necessary.
    #[inline]
    pub fn finish_task(&mut self, result: Pointer) -> bool {
        unsafe {
            // (note that there is a "-1" cell which is accessible even if there is only one item)
            let previous_cell = self.items.get_unchecked_mut(self.index_after_top - 2);
            if (*previous_cell as u32) == u32::MAX {
                // Previous cell is also result - just save.
                let cell = self.items.get_unchecked_mut(self.index_after_top - 1);
                *cell = (result | Pointer::undef()).0;
                true
            } else {
                // Previous cell is a task - we should swap them.
                let swapped_task = *previous_cell;
                *previous_cell = (result | Pointer::undef()).0;
                let cell = self.items.get_unchecked_mut(self.index_after_top - 1);
                *cell = swapped_task;
                false
            }
        }
    }

}

pub fn gen_tasks(left_bdd: &Bdd, right_bdd: &Bdd, task_cache: &mut TaskCache, node_cache: &mut PartialNodeCache) -> usize {
    let variable_count = max(left_bdd.variable_count(), right_bdd.variable_count());
    let larger_size = max(left_bdd.node_count(), right_bdd.node_count());

    let mut result = Bdd::new_true_with_variables(variable_count);
    //let node_cache = &mut PartialNodeCache::new(2 * larger_size);
    //let task_cache = &mut TaskCache::new(2 * larger_size);
    node_cache.clear();
    task_cache.clear();
    let stack = &mut UnrolledStack::new(2 * usize::from(variable_count));
    //stack.push_new_task((left.root_pointer(), right.root_pointer()), 0, false);
    //stack.reserved_push(left.root_pointer(), right.root_pointer(), 2);
    stack.push(left_bdd.root_pointer() | right_bdd.root_pointer());

    let mut iteration_count = 0;
    /*while let Some(task) = stack.pop() {
        iteration_count += 1;
        expand_task(task, &mut task_cache, &mut stack, left, right);
    }*/
    loop {
        let do_task = !stack.is_result(0);
        let mut and_pop = false;

        if do_task {
            let (left, right) = stack.peek_task();

            if left.is_zero() || right.is_one() {
                and_pop = and_pop | stack.finish_task(Pointer::zero());
            } else if left.is_one() && right.is_zero() {
                and_pop = and_pop | stack.finish_task(Pointer::one());
            } else {
                let cached_pointer = task_cache.read(left, right);
                if !cached_pointer.is_undef() {
                    and_pop = and_pop | stack.finish_task(cached_pointer);
                } else {
                    let (left_var, right_var) = (left_bdd.var_of(left), right_bdd.var_of(right));
                    let decision_variable = min(left_var, right_var);

                    let (l_low, l_high) = if left_var != decision_variable {
                        (left, left)
                    } else {
                        left_bdd.pointers_of(left).unpack()
                    };

                    let (r_low, r_high) = if right_var != decision_variable {
                        (right, right)
                    } else {
                        right_bdd.pointers_of(right).unpack()
                    };

                    stack.push(l_high | r_high);
                    stack.push(l_low | r_low);

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
                    }
                }
            }
        }

        if !do_task || and_pop {
            // Stack contains results for both tasks, we can finalize.
            let high = stack.pop_result();
            let low = stack.pop_result();
            let (left, right) = stack.peek_task();

            if high == low {
                task_cache.write(left, right, low);
                stack.finish_task(low);
            } else {
                let (left_var, right_var) = (left_bdd.var_of(left), right_bdd.var_of(right));
                let decision_variable = min(left_var, right_var);

                let mut result_pointer = node_cache.read(decision_variable, low | high, &result);
                if result_pointer.is_undef() {
                    result_pointer = result.create_node(decision_variable, low, high);
                    node_cache.write(decision_variable, low | high, result_pointer);
                }

                task_cache.write(left, right, result_pointer);
                stack.finish_task(result_pointer);
            }
        }

        if stack.len() == 1 {
            break;
        }
    }

    result.node_count()
}