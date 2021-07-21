use crate::v2::bench_fun::deps::{Bdd, NodeId};
use std::cmp::{min, max};
use std::collections::HashSet;
use fxhash::FxBuildHasher;
use std::num::NonZeroU64;
use std::convert::TryFrom;
use std::ops::{BitXor, Rem, Shr, Shl};
use likely_stable::unlikely;
use std::process::exit;
use lazy_static::lazy_static;

pub mod deps;

pub struct TaskSet {
    collisions: usize,
    capacity: NonZeroU64,
    keys: Vec<(NodeId, NodeId)>,
    //pressure: Vec<usize>,
}

impl TaskSet {
    const HASH_BLOCK: u64 = 1 << 14;
    pub const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(left_size: usize, right_size: usize) -> TaskSet {
        debug_assert!(left_size >= right_size);
        let capacity = max(left_size, right_size);
        TaskSet {
            collisions: 0,
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            keys: vec![(NodeId::ZERO, NodeId::ZERO); capacity],
            //pressure: vec![0; capacity],
        }
    }

    /// Return true if item was inserted.
    #[inline]
    pub fn ensure(&mut self, left: NodeId, right: NodeId) -> bool {
        let (index, random) = self.hashed_index(left, right);
        unsafe {
            let cell = self.keys.get_unchecked_mut(index);
            if *cell == (left, right) {
                false
            } /* else if *cell == (NodeId::ZERO, NodeId::ZERO) {
                *cell = (left, right);
                true
            } else {
                let cell = self.keys.get_unchecked_mut(random);
                if *cell == (left, right) {
                    false
                } else {
                    *cell = (left, right);
                    true
                }
            } */ else {
                *cell = (left, right);
                true
            }
        }
    }

    #[inline]
    fn hashed_index(&self, left: NodeId, right: NodeId) -> (usize, usize) {
        // Shift prevents collisions on queries with high number of left == right tasks.
        let left_hash = u64::from(left).rotate_left(7).wrapping_mul(Self::SEED);
        let right_hash = u64::from(right).wrapping_mul(Self::SEED);
        let block_index: u64 = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        let block_start: u64 = u64::from(left);// + u64::from(right).shr(10);
        ((block_start + block_index).rem(self.capacity) as usize, left_hash.bitxor(right_hash).rem(self.capacity) as usize)
        //left_hash.bitxor(right_hash).rem(self.capacity) as usize
    }

}

pub struct UnsafeStack {
    index_after_last: usize,
    items: Vec<(NodeId, NodeId)>
}

impl UnsafeStack {

    pub fn new(variable_count: u16) -> UnsafeStack {
        let capacity = 2 * usize::from(variable_count) + 2;
        let mut items = Vec::with_capacity(capacity);
        unsafe { items.set_len(items.capacity()); }
        UnsafeStack {
            items,
            index_after_last: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, left: NodeId, right: NodeId) {
        if left.is_terminal() && right.is_terminal() {
            return;
        }
        unsafe {
            let cell = self.items.get_unchecked_mut(self.index_after_last);
            *cell = (left, right);
            self.index_after_last += 1;
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index_after_last == 0
    }

    #[inline]
    pub fn pop(&mut self) -> (NodeId, NodeId) {
        self.index_after_last -= 1;
        unsafe { *self.items.get_unchecked_mut(self.index_after_last) }
    }

}

pub fn optimized_coupled_dfs(left: &Bdd, right: &Bdd) -> u64 {
    //let stack_capacity = 2 * usize::from(left.variable_count()) + 2;
    //let mut stack = Vec::with_capacity(stack_capacity);
    let mut stack = UnsafeStack::new(left.variable_count());
    stack.push(left.root_node(), right.root_node());

    let mut expanded = TaskSet::new(left.node_count(), right.node_count());

    let mut count = 0;
    loop {

        if unlikely(stack.is_empty()) {
            break;
        }

        let (left_pointer, right_pointer) = stack.pop();
        //println!("{} {}", u64::from(left_pointer), u64::from(right_pointer));

        if expanded.ensure(left_pointer, right_pointer) {
            /*if count % 100_000_000 == 0 {
                println!("Count: {} {}", count, expanded.collisions);
            }*/
            count += 1;
            let left_node = unsafe { left.get_node_unchecked(left_pointer) };
            let right_node = unsafe { right.get_node_unchecked(right_pointer) };

            if left_node.variable() == right_node.variable() {
                stack.push(left_node.high_link(), right_node.high_link());
                stack.push(left_node.low_link(), right_node.low_link());
            } else if left_node.variable() < right_node.variable() {
                stack.push(left_node.high_link(), right_pointer);
                stack.push(left_node.low_link(), right_pointer);
            } else {
                stack.push(left_pointer, right_node.high_link());
                stack.push(left_pointer, right_node.low_link());
            }


            /*let variable = min(left_node.variable(), right_node.variable());

            let (left_low, left_high) = if left_node.variable() == variable {
                left_node.links()
            } else {
                (left_pointer, left_pointer)
            };

            let (right_low, right_high) = if right_node.variable() == variable {
                right_node.links()
            } else {
                (right_pointer, right_pointer)
            };

            if !(left_high.is_terminal() && right_high.is_terminal()) {
                stack.push(left_high, right_high);
                //expanded.prefetch(left_high, right_high);
            }
            if !(left_low.is_terminal() && right_low.is_terminal()) {
                stack.push(left_low, right_low);
            }*/
        }
    }

    /*for (i, p) in expanded.pressure.iter().enumerate() {
        if *p > 3 {
            println!("{}: {}", i, *p);
        }
    }*/

    count
}

/// Naive coupled DFS implementation using the basic hash map.
pub fn naive_coupled_dfs(left: &Bdd, right: &Bdd) -> u64 {
    let stack_capacity = 2 * usize::from(left.variable_count()) + 2;
    let mut stack = Vec::with_capacity(stack_capacity);
    stack.push((left.root_node(), right.root_node()));

    let mut expanded = HashSet::with_capacity_and_hasher(max(left.node_count(), right.node_count()), FxBuildHasher::default());

    let mut count = 0;
    while let Some(top) = stack.pop() {
        if expanded.contains(&top) {
            continue;
        } else {
            expanded.insert(top);
            count += 1;
            let (left_pointer, right_pointer) = top;
            let left_node = unsafe { left.get_node_unchecked(left_pointer) };
            let right_node = unsafe { right.get_node_unchecked(right_pointer) };

            let variable = min(left_node.variable(), right_node.variable());

            let (left_low, left_high) = if left_node.variable() == variable {
                left_node.links()
            } else {
                (left_pointer, left_pointer)
            };

            let (right_low, right_high) = if right_node.variable() == variable {
                right_node.links()
            } else {
                (right_pointer, right_pointer)
            };

            if !(left_high.is_terminal() && right_high.is_terminal()) {
                stack.push((left_high, right_high));
            }
            if !(left_low.is_terminal() && right_low.is_terminal()) {
                stack.push((left_low, right_low));
            }
        }
    }

    count
}

/// A function for demonstrating the impact of pre-order on BDD traversal.
pub fn explore(bdd: &Bdd/*, stack: &mut Vec<NodeId>, visited: &mut Vec<bool>*/) -> u64 {

    let stack_capacity = 2 * usize::from(bdd.variable_count()) + 2;
    let mut stack: Vec<NodeId> = Vec::with_capacity(stack_capacity);
    unsafe { stack.set_len(stack_capacity) };
    let mut index_after_top = 1;
    unsafe { *stack.get_unchecked_mut(0) = bdd.root_node(); }

    let count = bdd.node_count();
    let mut visited = vec![false; count];
    /*for v in visited.iter_mut() {
        *v = false;
    }*/

    let mut count = 0;
    while index_after_top != 0 {
        index_after_top -= 1;
        let top = unsafe { *stack.get_unchecked(index_after_top) };
        let top_index = unsafe { top.as_index_unchecked() };
        let top_visited = unsafe { visited.get_unchecked_mut(top_index) };
        if !*top_visited {
            *top_visited = true;
            count += 1;
            let node = unsafe { bdd.get_node_unchecked(top) };
            unsafe {
                let high_link = node.high_link();
                if !high_link.is_zero() {
                    *stack.get_unchecked_mut(index_after_top) = high_link;
                    index_after_top += 1;
                }
                let low_link = node.low_link();
                if !low_link.is_zero() {
                    *stack.get_unchecked_mut(index_after_top) = low_link;
                    index_after_top += 1;
                }
                //*stack.get_unchecked_mut(index_after_top) = node.high_link();
                //*stack.get_unchecked_mut(index_after_top + 1) = node.low_link();
                //index_after_top += 2;
            }
        }
    }
    /*
    while let Some(top) = stack.pop() {
        let top_index = unsafe { top.as_index_unchecked() };
        if !visited[top_index] {
            visited[top_index] = true;
            count += 1;
            let node = unsafe { bdd.get_node_unchecked(top) };
            stack.push(node.high_link());
            stack.push(node.low_link());
        }
    }*/


    count
}