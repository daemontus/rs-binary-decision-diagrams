use crate::{Bdd, Pointer, PointerPair, SEED64, Variable};
use std::cmp::{max, min};
use std::num::NonZeroU64;
use std::ops::{Shl, Rem, Not, Shr};
use crate::_bdd_u32::PartialNodeCache;

pub struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<u64>,
    values: Vec<u32>,
}

impl TaskCache {

    pub fn new(capacity: usize) -> TaskCache {
        TaskCache {
            capacity: NonZeroU64::new(capacity as u64).unwrap(),
            keys: vec![u64::MAX; capacity],
            values: vec![0; capacity],
        }
    }

    pub fn clear(&mut self) {
        for i in &mut self.keys {
            *i = 0;
        }
    }

    #[inline]
    pub fn read(&self, x: Pointer, y: Pointer) -> u32 {
        let packed = x | y;
        let index = self.hash(packed);
        unsafe {
            if *self.keys.get_unchecked(index) == packed.0 {
                *self.values.get_unchecked(index)
            } else {
                u32::MAX
            }
        }
    }

    #[inline]
    pub fn write(&mut self, x: Pointer, y: Pointer, task_id: u32) {
        let packed = x | y;
        let index = self.hash(packed);
        unsafe {
            *self.keys.get_unchecked_mut(index) = packed.0;
            *self.values.get_unchecked_mut(index) = task_id;
        }
    }

    #[inline]
    pub fn hash(&self, pointers: PointerPair) -> usize {
        pointers.0.wrapping_mul(SEED64).rem(self.capacity) as usize
    }

}

pub struct UnrolledStack {
    index_after_top: usize,
    items: Vec<(Pointer, Pointer, u64)>,
}

impl UnrolledStack {

    pub fn new(capacity: usize) -> UnrolledStack {
        UnrolledStack {
            index_after_top: 0,
            items: vec![(Pointer::zero(), Pointer::zero(), 0); capacity],
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
    pub fn reserved_push(&mut self, x: Pointer, y: Pointer, parent_task: u64) {
        unsafe { *self.items.get_unchecked_mut(self.index_after_top) = (x, y, parent_task); }
        self.index_after_top += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.index_after_top
    }

    #[inline]
    pub fn pop(&mut self) -> (Pointer, Pointer, u64) {
        self.index_after_top -= 1;
        unsafe { *self.items.get_unchecked(self.index_after_top) }
    }

}

pub struct TaskQueue {
    index_after_last: usize,
    list_roots: Vec<u32>,
    items: Vec<u64>,
    successors: Vec<u32>,
}

impl TaskQueue {

    pub fn new(variables: u16, capacity: usize) -> TaskQueue {
        TaskQueue {
            index_after_last: 3,
            list_roots: vec![0; variables as usize],
            items: vec![0; capacity],
            successors: vec![0; capacity],
        }
    }

    /// Reserve capacity for at least 8 new tasks.
    #[inline]
    pub fn reserve_capacity_8(&mut self) {
        if self.items.len() < self.index_after_last + 8 {
            self.items.reserve(8);
            self.successors.reserve(8);
            unsafe { self.items.set_len(self.items.capacity()) };
            unsafe { self.successors.set_len(self.successors.capacity()) };
        }
    }

    /// Create a new task within a list of a specific variable and return its id.
    #[inline]
    pub fn create_task(&mut self, variable: Variable) -> u32 {
        unsafe {
            let current_list_root = *self.list_roots.get_unchecked(variable.0 as usize);
            let task_id = self.index_after_last;
            *self.successors.get_unchecked_mut(task_id) = current_list_root;
            *self.list_roots.get_unchecked_mut(variable.0 as usize) = task_id as u32;
            self.index_after_last += 1;
            task_id as u32
        }
    }

    #[inline]
    pub fn resolve_child(&mut self, parent_task: u64, child_id: u32) {
        let is_positive_child = (parent_task & 1) == 1;
        let parent_id = parent_task.shr(1);
        let child_id = child_id as u64;
        let update_mask = if is_positive_child { child_id.shl(32) } else { child_id };
        unsafe {
            let cell = self.items.get_unchecked_mut(parent_id as usize);
            *cell = *cell | update_mask;
        }
    }

}


pub fn gen_tasks(left: &Bdd, right: &Bdd, task_cache: &mut TaskCache, stack: &mut UnrolledStack) -> usize {
    let variable_count = max(left.variable_count(), right.variable_count());
    let larger_size = max(left.node_count(), right.node_count());

    let task_queue = &mut TaskQueue::new(variable_count, 3 * larger_size);
    let task_cache = &mut TaskCache::new(2 * larger_size);
    //task_cache.clear();
    let stack = &mut UnrolledStack::new(2 * usize::from(variable_count));//Vec::with_capacity(2 * usize::from(variable_count));
    stack.reserved_push(left.root_pointer(), right.root_pointer(), 2);

    let mut iteration_count = 0;
    /*while let Some(task) = stack.pop() {
        iteration_count += 1;
        expand_task(task, &mut task_cache, &mut stack, left, right);
    }*/
    loop {
        stack.reserve_capacity_16();
        task_queue.reserve_capacity_8();
        if stack.len() > 4 {
            iteration_count += 4;
            let task1 = stack.pop();
            let task2 = stack.pop();
            let task3 = stack.pop();
            let task4 = stack.pop();
            expand_task(task1, task_cache, task_queue, stack, left, right);
            expand_task(task2, task_cache, task_queue, stack, left, right);
            expand_task(task3, task_cache, task_queue, stack, left, right);
            expand_task(task4, task_cache, task_queue, stack, left, right);
        } else if stack.len() > 0 {
            iteration_count += 1;
            let task = stack.pop();
            expand_task(task, task_cache, task_queue, stack, left, right);
        } else {
            break;
        }
    }

    let mut result = Bdd::new_true_with_variables(variable_count);
    let mut node_cache = PartialNodeCache::new(task_queue.items.len() / 2);

    for v in (0..variable_count).rev() {
        let mut task_id = task_queue.list_roots[v as usize];
        while task_id != 0 {
            let task_index = task_id as usize;
            let dependencies = unsafe { *task_queue.items.get_unchecked(task_index) };
            let positive_dep = dependencies.shr(32) as u32;
            let negative_dep = dependencies as u32;
            let low_pointer = Pointer(unsafe { *task_queue.items.get_unchecked(negative_dep as usize) } as u32);
            let high_pointer = Pointer(unsafe { *task_queue.items.get_unchecked(positive_dep as usize) } as u32);
            let cached_node = node_cache.read(Variable(v), low_pointer | high_pointer, &result);
            if !cached_node.is_undef() {
                unsafe  { *task_queue.items.get_unchecked_mut(task_index) = cached_node.0 as u64; }
            } else {
                let node = result.create_node(
                    Variable(v),
                    Pointer(unsafe { *task_queue.items.get_unchecked(negative_dep as usize) } as u32),
                    Pointer(unsafe { *task_queue.items.get_unchecked(positive_dep as usize) } as u32),
                );
                node_cache.write(Variable(v), low_pointer | high_pointer, node);
                unsafe  { *task_queue.items.get_unchecked_mut(task_index) = node.0 as u64; }
            }
            iteration_count -= 1;
            task_id = unsafe { *task_queue.successors.get_unchecked(task_index) };
        }
    }

    result.node_count()
}

#[inline]
fn expand_task(
    task: (Pointer, Pointer, u64),
    task_cache: &mut TaskCache,
    task_queue: &mut TaskQueue,
    stack: &mut UnrolledStack,
    left: &Bdd,
    right: &Bdd,
) {
    let (l, r, parent_id) = task;

    // First, check if the task can be resolved using "terminal" rules.
    if l.is_zero() || r.is_one() {
        task_queue.resolve_child(parent_id, 0);
        return;
    }
    if l.is_one() && r.is_zero() {
        task_queue.resolve_child(parent_id, 1);
        return;
    }
    //if l.is_zero() || r.is_one() || (l.is_one() && r.is_zero()) {
    //    return;
    //}

    // Second, check if the task is saved in the cache. If yes, it is already expanded.
    let cached_task_id = task_cache.read(l, r);
    if cached_task_id != u32::MAX {
        task_queue.resolve_child(parent_id, cached_task_id);
        return;
    }
    //if task_cache.read(l, r) {
    //    return;
    //}

    // If none of it holds, process the task now:

    let (l_var, r_var) = (left.var_of(l), right.var_of(r));
    let decision_variable = min(l_var, r_var);

    let new_task_id = task_queue.create_task(decision_variable);
    task_cache.write(l, r, new_task_id);

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


    let parent_id = (new_task_id as u64).shl(1);
    stack.reserved_push(l_low, r_low, parent_id);
    stack.reserved_push(l_high, r_high, parent_id + 1);

    unsafe {
        let hash_low = task_cache.hash(l_low | r_low);
        let hash_high = task_cache.hash(l_high | r_high);
        let low_ref: *const u64 = task_cache.keys.get_unchecked(hash_low);
        let high_ref: *const u64 = task_cache.keys.get_unchecked(hash_high);
        std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
        std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

        let low_ref: *const u32 = task_cache.values.get_unchecked(hash_low);
        let high_ref: *const u32 = task_cache.values.get_unchecked(hash_high);
        std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
        std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));
/*
        let pointer: *const u64 = &left.nodes.get_unchecked(l_low.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &left.node_pointers.get_unchecked(l_high.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &right.node_pointers.get_unchecked(r_low.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
        let pointer: *const u64 = &right.node_pointers.get_unchecked(r_high.0 as usize).0;
        std::arch::x86_64::_mm_prefetch::<3>(pointer as (*const i8));
 */
    }
}