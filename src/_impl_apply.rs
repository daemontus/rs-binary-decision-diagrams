use crate::_impl_u8::u8_apply;
use crate::{Bdd, Pointer, PointerPair, Variable};
use fxhash::FxBuildHasher;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::ops::{Shl, Shr};

static FUNCTIONS: [fn(&Bdd, &Bdd) -> Bdd; 7] = [
    crate::_bdd_u16::and_not::<256, 1024, 1024>,
    crate::_bdd_u16::and_not::<256, 2048, 1024>,
    crate::_bdd_u16::and_not::<256, 4096, 1024>,
    crate::_bdd_u16::and_not::<256, 8192, 1024>,
    crate::_bdd_u16::and_not::<256, 16384, 1024>,
    crate::_bdd_u16::and_not::<256, 32768, 1024>,
    crate::_bdd_u16::and_not::<256, 35535, 1024>,
];

impl Bdd {
    pub fn and_not(&self, right: &Bdd) -> Bdd {
        let variables = max(self.variable_count(), right.variable_count());
        let max_input = max(self.node_count(), right.node_count());
        let expected_size = self.node_count() * right.node_count();
        if variables <= 128 && expected_size < usize::from(u16::MAX) {
            // This approach has a little less overhead than just putting in a ton of branches.
            if max_input < 1024 {
                // Convert to u16 and divide by 1024 - this should leave us somewhere between 0 and 64
                let magnitude: u16 = (expected_size as u16).shr(10);
                // Now compute "almost log2(magnitude)" which gives us an index into the array.
                let log_magnitude = 16 - magnitude.leading_zeros() as usize;
                return unsafe { FUNCTIONS.get_unchecked(log_magnitude)(self, right) };
            } else if max_input < 16386 {
                if expected_size < 1024 {
                    return crate::_bdd_u16::and_not::<2048, 1024, 16386>(self, right);
                } else if expected_size < 16384 {
                    return crate::_bdd_u16::and_not::<2048, 16384, 16386>(self, right);
                } else if expected_size < 65535 {
                    return crate::_bdd_u16::and_not::<2048, 65535, 16386>(self, right);
                }
            }
        }
        crate::_bdd_u32::and_not(self, right)
        //if self.node_count() <= usize::from(u8::MAX) && right.node_count() <= usize::from(u8::MAX) {
        //    u8_apply(self, right, crate::function::and_not)
        //} else {
        //apply(self, right, crate::function::and_not)
        //}
        /*let left = self;
        let mut result = Bdd::new_true();
        result.ensure_variables(variables);

        let mut result_is_empty = true;

        let mut stack: Vec<PointerPair> = Vec::new();
        stack.push(left.root_pointer() | right.root_pointer());

        // Maps a pair of pointers from the left/right Bdd to a pointer in the result Bdd.
        let mut task_cache: HashMap<PointerPair, Pointer, FxBuildHasher> = HashMap::with_capacity_and_hasher(
            max(left.node_count(), right.node_count()),
            FxBuildHasher::default(),
        );

        // Maps a known node to its pointer in the result Bdd.
        let mut node_cache: HashMap<(Variable, PointerPair), Pointer, FxBuildHasher> = HashMap::with_capacity_and_hasher(
            max(left.node_count(), right.node_count()),
            FxBuildHasher::default(),
        );
        node_cache.insert((result.node_variables[0], result.node_pointers[0]), Pointer::zero());
        node_cache.insert((result.node_variables[1], result.node_pointers[1]), Pointer::one());

        while let Some(on_stack) = stack.last() {
            if task_cache.contains_key(on_stack) {
                stack.pop();
            } else {
                let (l, r) = on_stack.unpack();

                let (l_v, r_v) = (left.var_of(l), right.var_of(r));
                let decision_var = min(l_v, r_v);

                let (l_low, l_high) = if l_v != decision_var {
                    (l, l)
                } else {
                    left.pointers_of(l).unpack()
                };
                let (r_low, r_high) = if r_v != decision_var {
                    (r, r)
                } else {
                    right.pointers_of(r).unpack()
                };

                let task_low = l_low | r_low;
                let task_high = l_high | r_high;

                // Inlined table for the and_not function
                // false & !??? = false
                // ??? & !true = false
                // true & !false = true
                // otherwise undefined...

                let new_low = if l_low.is_zero() || r_low.is_one() {
                    Some(Pointer::zero())
                } else if l_low.is_one() && r_low.is_zero() {
                    Some(Pointer::one())
                } else {
                    task_cache.get(&(l_low | r_low)).cloned()
                };
                let new_high = if l_high.is_zero() || r_high.is_one() {
                    Some(Pointer::zero())
                } else if l_high.is_one() && r_high.is_zero() {
                    Some(Pointer::one())
                } else {
                    task_cache.get(&(l_high | r_high)).cloned()
                };

                if let (Some(new_low), Some(new_high)) = (new_low, new_high) {
                    if new_low.is_one() || new_high.is_one() {
                        result_is_empty = false
                    }

                    if new_low == new_high {
                        // There is no decision, just skip this node and point to either child.
                        task_cache.insert(*on_stack, new_low);
                    } else {
                        // There is a decision here.
                        let node = (decision_var, new_low | new_high);
                        if let Some(index) = node_cache.get(&node) {
                            // Node already exists, just make it a result of this computation.
                            task_cache.insert(*on_stack, *index);
                        } else {
                            // Node does not exist, it needs to be pushed to result.
                            result.push_node(node.0, node.1);
                            node_cache.insert(node, result.root_pointer());
                            task_cache.insert(*on_stack, result.root_pointer());
                        }
                    }
                    stack.pop(); // Mark as resolved.
                } else {
                    // Otherwise, if either value is unknown, push it to the stack.
                    if new_low.is_none() {
                        stack.push(task_low);
                    }
                    if new_high.is_none() {
                        stack.push(task_high);
                    }
                }
            }
        }

        if result_is_empty {
            result = Bdd::new_false();
            result.ensure_variables(max(left.variable_count(), right.variable_count()));
            result
        } else {
            result
        }*/
    }
}
