use crate::v2::{Bdd, BddNode, NodeId};
use coupled_dfs_stack::Stack;
use partial_node_cache::NodeCache;
use partial_task_cache::TaskCache;
use std::cmp::{max, min};

/// **(internal)** A task/result stack used when performing the "coupled DFS" routine.
mod coupled_dfs_stack;

/// **(internal)** A partial task cache is an incomplete storage of task results.
mod partial_task_cache;

/// **(internal)** A partial node cache serves as incomplete storage for uniqueness resolution.
pub(super) mod partial_node_cache;

/// **(internal)** A general apply algorithm for performing arbitrary binary operations
/// on arbitrary `Bdd` objects.
///
/// The `TABLE` argument represents a "lookup table" which we use to resolve queries
/// on literals - it returns `NodeId::UNDEFINED` if the result cannot be resolved
/// into a terminal.
///
/// Note that the left `Bdd` must always be the larger one.
pub(super) fn _u48_apply<TABLE>(left_bdd: &Bdd, right_bdd: &Bdd, lookup: TABLE) -> Bdd
where
    TABLE: Fn(NodeId, NodeId) -> NodeId,
{
    let variables = max(left_bdd.variable_count(), right_bdd.variable_count());

    let mut is_not_false = false;
    let mut node_cache = NodeCache::new(left_bdd.node_count());
    let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());
    let mut stack = Stack::new(variables);
    unsafe {
        stack.push_task_unchecked(left_bdd.root_node(), right_bdd.root_node());
    }

    // The calls to stack operations are safe due to the order in which we perform the Bdd search.
    loop {
        // If the top is a result, go straight to finishing a task. If not, first expand,
        // but if the result of the expansion is a finished task, then also finish a task.
        let mut finish_task = stack.has_result();

        if !finish_task {
            // Expand current top task.
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            let lookup_result = lookup(left, right);
            is_not_false = is_not_false || lookup_result.is_one();

            if !lookup_result.is_undefined() {
                finish_task = finish_task || unsafe { stack.save_result_unchecked(lookup_result) };
            } else {
                let cached_node = task_cache.read(left, right);
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

                    task_cache.prefetch(left_high, right_high);

                    // When completed, the order of tasks will be swapped (high on top).
                    unsafe {
                        stack.push_task_unchecked(left_high, right_high);
                        stack.push_task_unchecked(left_low, right_low);
                    }
                }
            }
        }

        if finish_task {
            // Finish current top task.
            let (low, high) = unsafe { stack.pop_results_unchecked() };
            let (left, right) = unsafe { stack.peek_as_task_unchecked() };

            if high == low {
                task_cache.write(left, right, low);
                unsafe { stack.save_result_unchecked(low) };
            } else {
                let (left_var, right_var) =
                    (left_bdd.get_variable(left), right_bdd.get_variable(right));
                let decision_variable = min(left_var, right_var);

                let node = BddNode::pack(decision_variable, low, high);
                let result_id = node_cache.ensure(node);
                task_cache.write(left, right, result_id);
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

/// **(internal)** I apologise profoundly for this, but this seems to be a consistently faster
/// approach than using a lookup table, so we use this macro to implement the pre-defined binary
/// operations.
///
/// It is the general apply function but with the lookup table explicitly inlined using a macro.
macro_rules! apply_u48 {
    ($left:ident, $right:ident, $zero:expr, $one:expr) => {{
        let left_bdd = $left;
        let right_bdd = $right;
        let variables = max(left_bdd.variable_count(), right_bdd.variable_count());

        let mut is_not_false = false;
        let mut node_cache = NodeCache::new(left_bdd.node_count());
        let mut task_cache = TaskCache::new(left_bdd.node_count(), right_bdd.node_count());
        let mut stack = Stack::new(variables);
        unsafe { stack.push_task_unchecked(left_bdd.root_node(), right_bdd.root_node()); }

        // The calls to stack operations are safe due to the order in which we perform the Bdd search.
        loop {
            // If the top is a result, go straight to finishing a task. If not, first expand,
            // but if the result of the expansion is a finished task, then also finish a task.
            let mut finish_task = stack.has_result();

            if !finish_task {
                // Expand current top task.
                let (left, right) = unsafe { stack.peek_as_task_unchecked() };

                if $zero(left, right) {
                    finish_task = finish_task || unsafe { stack.save_result_unchecked(NodeId::ZERO) };
                } else if $one(left, right) {
                    is_not_false = true;
                    finish_task = finish_task || unsafe { stack.save_result_unchecked(NodeId::ONE) };
                } else {
                    let cached_node = task_cache.read(left, right);
                    if !cached_node.is_undefined() {
                        finish_task = finish_task || unsafe { stack.save_result_unchecked(cached_node) };
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

                        task_cache.prefetch(left_high, right_high);

                        // When completed, the order of tasks will be swapped (high on top).
                        unsafe {
                            stack.push_task_unchecked(left_high, right_high);
                            stack.push_task_unchecked(left_low, right_low);
                        }
                    }
                }
            }

            if finish_task {
                // Finish current top task.
                let (low, high) = unsafe { stack.pop_results_unchecked() };
                let (left, right) = unsafe { stack.peek_as_task_unchecked() };

                if high == low {
                    task_cache.write(left, right, low);
                    unsafe { stack.save_result_unchecked(low) };
                } else {
                    let (left_var, right_var) =
                        (left_bdd.get_variable(left), right_bdd.get_variable(right));
                    let decision_variable = min(left_var, right_var);

                    let node = BddNode::pack(decision_variable, low, high);
                    let result_id = node_cache.ensure(node);
                    task_cache.write(left, right, result_id);
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
    }}
}

/// **(internal)** Macro-generated general functions for mainstream logical operators.
///
/// We have a mirror operation for each asymmetric operation, because the main algorithm
/// assumes that the left Bdd is always the larger one.
impl Bdd {
    pub(super) fn _u48_and(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_zero() || r.is_zero(),
            |l: NodeId, r: NodeId| l.is_one() && r.is_one()
        )
    }

    pub(super) fn _u48_or(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_zero() && r.is_zero(),
            |l: NodeId, r: NodeId| l.is_one() || r.is_one()
        )
    }

    pub(super) fn _u48_imp(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_one() && r.is_zero(),
            |l: NodeId, r: NodeId| l.is_zero() || r.is_one()
        )
    }

    pub(super) fn _u48_inv_imp(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_zero() || r.is_one(),
            |l: NodeId, r: NodeId| l.is_one() && r.is_zero()
        )
    }

    pub(super) fn _u48_iff(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| (l.is_one() && r.is_zero()) || (l.is_zero() && r.is_one()),
            |l: NodeId, r: NodeId| (l.is_zero() && r.is_zero()) || (l.is_one() && r.is_one())
        )
    }

    pub(super) fn _u48_xor(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| (l.is_one() && r.is_one()) || (l.is_zero() && r.is_zero()),
            |l: NodeId, r: NodeId| (l.is_zero() && r.is_one()) || (l.is_one() && r.is_zero())
        )
    }

    pub(super) fn _u48_and_not(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_zero() || r.is_one(),
            |l: NodeId, r: NodeId| l.is_one() && r.is_zero()
        )
    }

    pub(super) fn _u48_not_and(&self, other: &Bdd) -> Bdd {
        debug_assert!(self.node_count() >= other.node_count());
        apply_u48!(
            self,
            other,
            |l: NodeId, r: NodeId| l.is_one() && r.is_zero(),
            |l: NodeId, r: NodeId| l.is_zero() || r.is_one()
        )
    }
}
