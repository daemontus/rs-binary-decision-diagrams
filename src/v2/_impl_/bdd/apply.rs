use crate::v2::{Bdd, BddNode, NodeId, VariableId};
use std::cmp::{max, min};
use std::num::NonZeroU64;
use std::ops::{BitXor, Rem, Shl};

/// **(internal)** A stack keeps track of tasks that still need to be completed or the result of
/// which has not been used yet. Each entry has either two valid left/right `NodeId` pointers,
/// or a single `NodeId::UNDEFINED` and a valid result pointer.
///
/// The special feature of the stack is that when you replace the top task with a result value,
/// it will automatically swap with an entry underneath if that entry is not result as well.
/// This mechanism ensures that if the top entry is a result, we know that the entry underneath
/// is a result as well and we can finish the task that spawned them.
struct Stack {
    index_after_last: usize,
    items: Vec<(NodeId, NodeId)>,
}

impl Stack {
    /// Create a new stack with a sufficient capacity for a "coupled DFS" over `Bdds` with
    /// depth bounded by `variable_count`.
    pub fn new(variable_count: u16) -> Stack {
        let variable_count = usize::from(variable_count);
        let mut stack = Stack {
            index_after_last: 1,
            // In a standard "coupled DFS" algorithm, the stack can never be larger than
            // 2 * the number of variables in the Bdd.
            items: vec![(NodeId::ZERO, NodeId::ZERO); 2 * variable_count + 2],
        };
        // A "fake" first entry ensures that even when the last task finishes, we can safely
        // check the predecessor. It does not count into the length of the stack.
        // Since the fake entry is a result, they will not swap.
        stack.items[0] = (NodeId::UNDEFINED, NodeId::ZERO);
        stack
    }

    /// True if the stack has only one entry. This is actually the terminating condition
    /// for the search because in a do-while loop the last entry must be a result.
    #[inline]
    pub fn has_one_entry(&self) -> bool {
        self.index_after_last == 2
    }

    /// Create a new task on stack.
    #[inline]
    pub fn push_task(&mut self, left: NodeId, right: NodeId) {
        debug_assert!(self.index_after_last < self.items.len());
        unsafe { *self.items.get_unchecked_mut(self.index_after_last) = (left, right) }
        self.index_after_last += 1;
    }

    /// True if the top item is a result.
    #[inline]
    pub fn has_result(&self) -> bool {
        debug_assert!(self.index_after_last > 1);
        unsafe {
            self.items
                .get_unchecked(self.index_after_last - 1)
                .0
                .is_undefined()
        }
    }

    /// Pop an entry off the stack, interpreting it as a result id.
    ///
    /// Note that this does not check if the top is actually a result!
    #[inline]
    pub fn pop_results(&mut self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 2);
        self.index_after_last -= 2;
        unsafe {
            let y = self.items.get_unchecked(self.index_after_last + 1).1;
            let x = self.items.get_unchecked(self.index_after_last).1;
            (x, y)
        }
    }

    /// Get the top entry without popping it, interpreting it as a task.
    ///
    /// Note that this does not check if the top is actually a task!
    #[inline]
    pub fn peek_as_task(&self) -> (NodeId, NodeId) {
        debug_assert!(self.index_after_last > 1);
        unsafe { *self.items.get_unchecked(self.index_after_last - 1) }
    }

    /// Replace the top of the stack with a result entry. Return true if the top of the
    /// stack is now a result, or false if a task entry has been swapped on top.
    #[inline]
    pub fn save_result(&mut self, result: NodeId) -> bool {
        debug_assert!(self.index_after_last >= 2);
        // This operation is safe because we have that dummy first entry that gets accessed here.
        let before_top = unsafe { self.items.get_unchecked_mut(self.index_after_last - 2) };
        //let swap_on_top = *before_top;
        //*before_top = (NodeId::UNDEFINED, result);
        //unsafe { *self.items.get_unchecked_mut(self.index_after_last - 1) = swap_on_top; }
        //swap_on_top.0.is_undefined()
        if before_top.0.is_undefined() {
            // entry[-2] is also a result - just replace the top
            unsafe {
                *self.items.get_unchecked_mut(self.index_after_last - 1) =
                    (NodeId::UNDEFINED, result);
            }
            true
        } else {
            // entry[-2] is a task - swap it on top
            let swap_on_top = *before_top;
            *before_top = (NodeId::UNDEFINED, result);
            unsafe {
                *self.items.get_unchecked_mut(self.index_after_last - 1) = swap_on_top;
            }
            false
        }
    }
}

/// **(internal)** A partial hash map which handles uniqueness queries for the nodes of a `Bdd`.
///
/// It is a hash map which overwrites on collision, just as `TaskCache`, but it keeps the keys
/// in the result `Bdd`, avoiding double allocation. We also assume that `NodeId::ZERO` is never
/// saved into the cache (since it has a static position) and thus we can use it as an undefined
/// value.
struct NodeCache {
    capacity: NonZeroU64,
    values: Vec<NodeId>,
}

impl NodeCache {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(capacity: usize) -> NodeCache {
        debug_assert!(capacity > 0);
        NodeCache {
            capacity: unsafe { NonZeroU64::new_unchecked(capacity as u64) },
            values: vec![NodeId::ZERO; capacity],
        }
    }

    #[inline]
    pub fn ensure(&mut self, keys: &mut Bdd, node: BddNode) -> NodeId {
        let index = self.hash(node.0, node.1);
        unsafe {
            let entry = unsafe { self.values.get_unchecked_mut(index) };
            let candidate = *entry;
            //let node = BddNode::pack(variable, low, high);
            if !candidate.is_zero() && keys.get_node(candidate) == node {
                candidate
            } else {
                let new_id = keys.push_node(node);
                *entry = new_id;
                new_id
            }
        }
    }

    #[inline]
    pub fn prefetch(&self, low: NodeId, high: NodeId) {
        let index = self.hash(low.0, high.0);
        unsafe {
            let entry: *const NodeId = self.values.get_unchecked(index);
            std::arch::x86_64::_mm_prefetch::<3>(entry as *const i8);
        }
    }

    #[inline]
    fn hash(&self, low: u64, high: u64) -> usize {
        // Our hash function ignores the node variable at the moment. The reasoning is
        // that if we want to use prefetching with this cache, we need it to only depend
        // on the pointers as these are available, variable is not.
        let left = low.wrapping_mul(Self::SEED);
        let right = high.wrapping_mul(Self::SEED);
        left.bitxor(right).rem(self.capacity) as usize
    }
}

/// **(internal)** A partial hash map which saves the results of already processed tasks.
///
/// It is essentially a hash map which overwrites on collision to avoid costly ranches.
/// It relies on the fact that task (0,0) should be always resolved using a lookup table
/// and will therefore never appear as a key in the cache. This way, we can start by
/// zeroing all the memory, which appears to be slightly faster on x86 for some reason.
struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<(NodeId, NodeId)>,
    values: Vec<NodeId>,
}

impl TaskCache {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    pub fn new(capacity: usize) -> TaskCache {
        debug_assert!(capacity > 0);
        TaskCache {
            capacity: unsafe { NonZeroU64::new_unchecked(capacity as u64) },
            keys: vec![(NodeId::ZERO, NodeId::ZERO); capacity],
            values: vec![NodeId::ZERO; capacity],
        }
    }

    #[inline]
    pub fn read(&self, left: NodeId, right: NodeId) -> NodeId {
        let index = self.hash(left, right);
        unsafe {
            if *self.keys.get_unchecked(index) == (left, right) {
                *self.values.get_unchecked(index)
            } else {
                NodeId::UNDEFINED
            }
        }
    }

    #[inline]
    pub fn write(&mut self, left: NodeId, right: NodeId, result: NodeId) {
        let index = self.hash(left, right);
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            let value = self.values.get_unchecked_mut(index);
            *key = (left, right);
            *value = result;
        }
    }

    #[inline]
    pub fn prefetch(&self, left: NodeId, right: NodeId) {
        if cfg!(target_arch = "x86_64") {
            let index = self.hash(left, right);
            unsafe {
                let key: *const (NodeId, NodeId) = self.keys.get_unchecked(index);
                let value: *const (NodeId) = self.values.get_unchecked(index);
                std::arch::x86_64::_mm_prefetch::<3>(key as *const i8);
                std::arch::x86_64::_mm_prefetch::<3>(value as *const i8);
            }
        }
    }

    #[inline]
    fn hash(&self, left: NodeId, right: NodeId) -> usize {
        let left = left.0.wrapping_mul(Self::SEED);
        let right = right.0.wrapping_mul(Self::SEED);
        left.bitxor(right).rem(self.capacity) as usize
    }
}

macro_rules! generic_apply {
    ($left:ident, $right:ident, $zero:expr, $one:expr) => {{
        let left_bdd = $left;
        let right_bdd = $right;
        let variables = max(left_bdd.variable_count(), right_bdd.variable_count());
        let expected_size = max(left_bdd.node_count(), right_bdd.node_count());

        let mut result = Bdd::new_with_capacity(variables, expected_size);
        let mut is_false = true;

        let mut node_cache = NodeCache::new(expected_size);
        let mut task_cache = TaskCache::new(expected_size);
        let mut stack = Stack::new(variables);
        stack.push_task(left_bdd.root_node(), right_bdd.root_node());

        loop {
            // If the top is a result, go straight to finishing a task. If not, first expand,
            // but if the result of the expansion is a finished task, then also finish a task.
            let mut finish_task = stack.has_result();

            if !finish_task {
                // Expand current top task.
                let (left, right) = stack.peek_as_task();

                if $zero(left, right) {
                    finish_task = finish_task || stack.save_result(NodeId::ZERO);
                } else if $one(left, right) {
                    is_false = false;
                    finish_task = finish_task || stack.save_result(NodeId::ONE);
                } else {
                    let cached_node = task_cache.read(left, right);
                    if !cached_node.is_undefined() {
                        finish_task = finish_task || stack.save_result(cached_node);
                    } else {
                        let (left_var, left_low, left_high) = left_bdd.get_node(left).unpack();
                        let (right_var, right_low, right_high) = right_bdd.get_node(right).unpack();
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
                        stack.push_task(left_high, right_high);
                        stack.push_task(left_low, right_low);
                    }
                }
            }

            if finish_task {
                // Finish current top task.
                let (low, high) = stack.pop_results(&task_cache);
                //let (high, low) = stack.pop_results(&task_cache);
                let (left, right) = stack.peek_as_task();

                if high == low {
                    task_cache.write(left, right, low);
                    stack.save_result(low);
                } else {
                    let (left_var, right_var) =
                        (left_bdd.get_variable(left), right_bdd.get_variable(right));
                    let decision_variable = min(left_var, right_var);

                    let result_id = node_cache.ensure(&mut result, decision_variable, low, high);
                    task_cache.write(left, right, result_id);
                    stack.save_result(result_id);
                }
            }

            if stack.has_one_entry() {
                break; // The last entry is the result to the first task.
            }
        }

        if is_false {
            Bdd::new_false()
        } else {
            result
        }
    }};
}

pub fn and_not(left_bdd: &Bdd, right_bdd: &Bdd) -> Bdd {
    /*generic_apply!(left_bdd, right_bdd,
        |left: NodeId, right: NodeId| (left.is_zero() || right.is_one()),
        |left: NodeId, right: NodeId| (left.is_one() && right.is_zero())
    )*/
    let variables = max(left_bdd.variable_count(), right_bdd.variable_count());
    let expected_size = max(left_bdd.node_count(), right_bdd.node_count());

    let mut result = Bdd::true_with_capacity(expected_size);
    result.update_variable_count(variables);
    let mut is_false = true;

    let mut node_cache = NodeCache::new(expected_size); //super::binary_operations::u48::partial_node_cache::NodeCache::new(expected_size);
    let mut task_cache = TaskCache::new(expected_size);
    let mut stack = Stack::new(variables);
    stack.push_task(left_bdd.root_node(), right_bdd.root_node());

    loop {
        // If the top is a result, go straight to finishing a task. If not, first expand,
        // but if the result of the expansion is a finished task, then also finish a task.
        let mut finish_task = stack.has_result();

        if !finish_task {
            // Expand current top task.
            let (left, right) = stack.peek_as_task();

            if left.is_zero() || right.is_one() {
                finish_task = finish_task || stack.save_result(NodeId::ZERO);
            } else if left.is_one() && right.is_zero() {
                is_false = false;
                finish_task = finish_task || stack.save_result(NodeId::ONE);
            } else {
                let cached_node = task_cache.read(left, right);
                if !cached_node.is_undefined() {
                    finish_task = finish_task || stack.save_result(cached_node);
                } else {
                    let (left_var, left_low, left_high) = left_bdd.get_node(left).unpack();
                    let (right_var, right_low, right_high) = right_bdd.get_node(right).unpack();
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
                    stack.push_task(left_high, right_high);
                    stack.push_task(left_low, right_low);
                }
            }
        }

        if finish_task {
            // Finish current top task.
            let (low, high) = stack.pop_results();
            let (left, right) = stack.peek_as_task();

            if high == low {
                task_cache.write(left, right, low);
                stack.save_result(low);
            } else {
                let (left_var, right_var) =
                    (left_bdd.get_variable(left), right_bdd.get_variable(right));
                let decision_variable = min(left_var, right_var);

                let node = BddNode::pack(decision_variable, low, high);
                let result_id = node_cache.ensure(&mut result, node);
                task_cache.write(left, right, result_id);
                stack.save_result(result_id);
            }
        }

        if stack.has_one_entry() {
            break; // The last entry is the result to the first task.
        }
    }

    if is_false {
        Bdd::new_false()
    } else {
        //node_cache.export()
        result
    }
}
