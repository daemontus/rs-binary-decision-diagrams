#![allow(unused_imports)]

use crate::v2::bench_fun::deps::{Bdd, NodeId, BddNode, VariableId};
use std::cmp::{min, max};
use std::collections::{HashSet, HashMap};
use fxhash::FxBuildHasher;
use coupled_dfs::{TaskSet, UnsafeStack};
use crate::v2::bench_fun::apply::{Stack, TaskCache, NodeCache2};
use std::ops::{BitXor, Rem};
use biodivine_lib_bdd::op_function::and;
use std::num::NonZeroU64;

pub mod deps;
pub mod coupled_dfs;
pub mod apply;

const VARIABLE_MASK: u64 = (u16::MAX as u64) << 48;
const ID_MASK: u64 = !VARIABLE_MASK;

pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> Bdd {
    let variables = left_bdd.variable_count();
    let mut stack = Stack::new(left_bdd.variable_count());
    unsafe { stack.push_task_unchecked(left_bdd.root_node(), right_bdd.root_node()); }

    let mut node_cache = NodeCache2::new(left_bdd.node_count());

    //let mut nodes = Vec::with_capacity(left_bdd.node_count() + right_bdd.node_count());
    //nodes.push(BddNode(VariableId(0), NodeId::ZERO, NodeId::ZERO));
    //nodes.push(BddNode(VariableId(0), NodeId::ONE, NodeId::ONE));

    let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());

    loop {
        let mut has_result = stack.has_result();

        if !has_result {
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            if left.is_one() || right.is_one() {
                has_result = unsafe { stack.save_result_unchecked(NodeId::ONE) };
            } else if left.is_zero() && right.is_zero() {
                has_result = unsafe { stack.save_result_unchecked(NodeId::ZERO) };
            } else {
                let (cached_node, save_at) = task_cache.read(left, right);
                if !cached_node.is_undefined() {
                    has_result = unsafe { stack.save_result_unchecked(cached_node) };
                } else {
                    let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                    let right_node = unsafe { right_bdd.get_node_unchecked(right) };

                    let decision_variable = min(left_node.variable(), right_node.variable());

                    let (left_low, left_high) = if decision_variable == left_node.variable() {
                        left_node.links()
                    } else {
                        (left, left)
                    };

                    let (right_low, right_high) = if decision_variable == right_node.variable() {
                        right_node.links()
                    } else {
                        (right, right)
                    };

                    // When completed, the order of tasks will be swapped (high on top).
                    unsafe {
                        stack.push_task_unchecked(NodeId::ONE, NodeId(save_at));
                        stack.push_task_unchecked(left_high, right_high);
                        stack.push_task_unchecked(left_low, right_low);
                    }
                }
            }
        }

        if has_result {
            let (low, high) = unsafe { stack.pop_results_unchecked() };
            let (_, NodeId(save_at)) = unsafe { stack.pop_as_task_unchecked() };
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            /*let result = NodeId(low.0.bitxor(high.0) + count);  // Some bullshit just to not make it all zero
            //task_cache.write(left, right, result);
            task_cache.write_at(left, right, save_at.0, result);
            unsafe { stack.save_result_unchecked(result); }*/
            if high == low {
                task_cache.write_at(left, right, save_at, low);
                unsafe { stack.save_result_unchecked(low) };
            } else {
                let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                let right_node = unsafe { right_bdd.get_node_unchecked(right) };
                let decision_variable = min(left_node.variable(), right_node.variable());

                let node = BddNode(decision_variable, low, high);
                //let result_id = NodeId((nodes.len() - 1) as u64);
                //nodes.push(node);
                //println!("{} {}", low.0, high.0);
                let result_id = node_cache.ensure(node);
                task_cache.write_at(left, right, save_at, result_id);
                unsafe { stack.save_result_unchecked(result_id) };
            }
        }

        if stack.has_last_entry() {
            break;
        }
    }

    /*for (i, node) in nodes.iter().enumerate() {
        if i + 10 < nodes.len() {
            node_cache.prefetch(unsafe { nodes.get_unchecked(i + 10) });
        }
        node_cache.ensure(*node);
    }*/

    /*
    let capacity = NonZeroU64::new(nodes.len() as u64).unwrap();
    let mut hashes = Vec::with_capacity(nodes.len());
    unsafe { hashes.set_len(nodes.len()); }
    //let mut h: usize = 0;
    for (i, node) in nodes.iter().enumerate() {
        let low_hash = node.low_link().0.wrapping_mul(NodeCache2::SEED);
        let high_hash = node.high_link().0.wrapping_mul(NodeCache2::SEED);
        unsafe {
            *hashes.get_unchecked_mut(i) = low_hash.bitxor(high_hash).rem(capacity) as usize;
        }
    }*/

    //panic!("Collisions: {}", node_cache.collisions);

    let mut nodes = node_cache.nodes;
    let node_count = node_cache.index_after_last;

    for (_, i) in nodes.iter_mut() {
        *i = 0;
    }
    // First two entries are reserved for terminals:
    nodes[0] = ((0,0), 0);
    nodes[1] = ((1,1), 1);

    let mut new_index = 2;

    let new_root = stack.items[0].1;
    let mut stack = Vec::with_capacity(2 * usize::from(variables));
    unsafe { stack.set_len(stack.capacity()) };
    stack[0] = new_root;
    let mut index_after_last = 1;

    while index_after_last > 0 {
        index_after_last -= 1;
        // Unpack node
        let top = unsafe { *stack.get_unchecked(index_after_last) };
        let node_data = unsafe { nodes.get_unchecked_mut(top.as_index_unchecked()) };
        let (low, high) = (NodeId(node_data.0.0 & ID_MASK), NodeId(node_data.0.1));

        // Save index
        node_data.1 = new_index;
        new_index += 1;

        // Push new items on search stack
        if !high.is_terminal() {
            let high_node = unsafe { nodes.get_unchecked_mut(high.as_index_unchecked()) };
            if high_node.1 == 0 {
                unsafe {
                    *stack.get_unchecked_mut(index_after_last) = high;
                    index_after_last += 1;
                }
            }
        }

        if !low.is_terminal() {
            let low_node = unsafe { nodes.get_unchecked_mut(low.as_index_unchecked()) };
            if low_node.1 == 0 {
                unsafe {
                    *stack.get_unchecked_mut(index_after_last) = low;
                    index_after_last += 1;
                }
            }
        }
    }

    let mut new_nodes = Vec::with_capacity(node_count + 1);
    new_nodes.push(BddNode(VariableId::UNDEFINED, NodeId::ZERO, NodeId::ZERO));
    new_nodes.push(BddNode(VariableId::UNDEFINED, NodeId::ONE, NodeId::ONE));
    unsafe { new_nodes.set_len(node_count) };

    for i in 2..node_count {
        let original_node = unsafe { nodes.get_unchecked(i) };
        let variable = ((original_node.0.0 & VARIABLE_MASK) >> 48) as u16;
        let (low, high) = (NodeId(original_node.0.0 & ID_MASK), NodeId(original_node.0.1));

        let new_low_id =  unsafe { NodeId(nodes.get_unchecked(low.as_index_unchecked()).1) };
        let new_high_id = unsafe { NodeId(nodes.get_unchecked(high.as_index_unchecked()).1) };

        let my_new_id = NodeId(original_node.1);

        unsafe {
            *new_nodes.get_unchecked_mut(my_new_id.as_index_unchecked()) = BddNode(VariableId(variable), new_low_id, new_high_id);
        }
    }

    Bdd {
        variable_count: variables,
        nodes: new_nodes
    }
    //hashes.len() as u64
    //count
    //nodes.len() as u64
    //node_cache.index_after_last as u64
}

pub fn naive_apply(left_bdd: &Bdd, right_bdd: &Bdd) -> u64 {
    let variables = max(left_bdd.variable_count(), right_bdd.variable_count());

    let mut result_nodes = Vec::new();
    result_nodes.push(BddNode(VariableId(variables), NodeId::ZERO, NodeId::ZERO));
    result_nodes.push(BddNode(VariableId(variables), NodeId::ONE, NodeId::ONE));
    //let mut is_not_false = false;
    let mut node_cache: HashMap<BddNode, NodeId, FxBuildHasher> = HashMap::with_capacity_and_hasher(left_bdd.node_count(), FxBuildHasher::default());
    let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());
    let mut stack = Stack::new(variables);
    unsafe {
        stack.push_task_unchecked(left_bdd.root_node(), right_bdd.root_node());
    }

    //let mut result_id = 0;
    let mut task_count = 0;
    loop {
        if stack.has_result() {
            // Finish current top task.
            let (low, high) = unsafe { stack.pop_results_unchecked() };
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            if high == low {
                task_cache.write(left, right, low);
                unsafe { stack.save_result_unchecked(low) };
            } else {
                let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                let right_node = unsafe { right_bdd.get_node_unchecked(right) };
                let decision_variable = min(left_node.variable(), right_node.variable());

                let node = BddNode(decision_variable, low, high);
                let result_id = if let Some(id) = node_cache.get(&node) {
                    *id
                } else {
                    let id = NodeId(result_nodes.len() as u64);
                    node_cache.insert(node, id);
                    result_nodes.push(node);
                    id
                };
                task_cache.write(left, right, result_id);
                unsafe { stack.save_result_unchecked(result_id) };
            }
        } else {
            // Expand current top task.
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            if left.is_one() || right.is_one() {
                unsafe { stack.save_result_unchecked(NodeId::ONE); }
                //is_not_false = true;
            } else if left.is_zero() && right.is_zero() {
                unsafe { stack.save_result_unchecked(NodeId::ZERO); }
            } else {
                let (cached_node, _) = task_cache.read(left, right);
                if !cached_node.is_undefined() {
                    unsafe { stack.save_result_unchecked(cached_node); }
                } else {
                    task_count += 1;
                    let left_node = unsafe { left_bdd.get_node_unchecked(left) };
                    let right_node = unsafe { right_bdd.get_node_unchecked(right) };


                    let decision_variable = min(left_node.variable(), right_node.variable());

                    let (left_low, left_high) = if decision_variable == left_node.variable() {
                        left_node.links()
                    } else {
                        (left, left)
                    };

                    let (right_low, right_high) = if decision_variable == right_node.variable() {
                        right_node.links()
                    } else {
                        (right, right)
                    };

                    // When completed, the order of tasks will be swapped (high on top).
                    unsafe {
                        stack.push_task_unchecked(left_high, right_high);
                        stack.push_task_unchecked(left_low, right_low);
                    }


                }
            }
        }

        if stack.has_last_entry() {
            break; // The last entry is the result to the first task.
        }
    }

    task_count
    //node_cache.len() as u64
}

pub fn optimized_coupled_dfs(left: &Bdd, right: &Bdd) -> u64 {
    //let stack_capacity = 2 * usize::from(left.variable_count()) + 2;
    //let mut stack = Vec::with_capacity(stack_capacity);
    let mut stack = UnsafeStack::new(left.variable_count());
    stack.push(left.root_node(), right.root_node());

    let mut expanded = TaskSet::new(left.node_count(), right.node_count());

    let mut count = 0;
    loop {

        if stack.is_empty() {
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