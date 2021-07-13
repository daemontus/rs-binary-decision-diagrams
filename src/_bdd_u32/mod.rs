use crate::{Pointer, Variable, PointerPair};
use std::num::NonZeroU64;

mod _impl_partial_task_cache;
mod _impl_partial_node_cache;
mod _impl_complete_task_queue;
mod _impl_apply;
pub mod _impl_task_bench;
pub mod _impl_task_bench_2;
pub mod _impl_task_bench_3;

pub use _impl_apply::and_not;

/// Partial task cache is a small "incomplete" hashed cache which is used to avoid
/// re-evaluating most tasks. However, it can't be used to evaluate the tasks because
/// it is incomplete.
struct PartialTaskCache {
    capacity: NonZeroU64,
    keys: Vec<(Pointer, Pointer)>,
    values: Vec<usize>,
}

/// Compared to `PartialTaskCache`, a task queue is complete and it maintains exactly
/// as many linked lists of tasks as the BDDs have variables, such that they can then
/// be evaluated based on this order
struct CompleteTaskQueue {
    list_roots: Vec<usize>,
    // The pair are the two tasks this task depends on, the last number is the next task in this level.
    tasks: Vec<Task>,
}

#[derive(Copy, Clone)]
struct Task {
    dependencies: (usize, usize),
    result: Pointer,
    next_task: usize,
}

pub struct PartialNodeCache {
    capacity: NonZeroU64,
    //keys: Vec<(Variable, PointerPair)>,
    values: Vec<Pointer>,
}