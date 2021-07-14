use crate::Bdd;
use crate::_bdd_u16::{NodeU64, PointerU16, StaticNodeCache, StaticStack, StaticTaskCache};
use std::cmp::{max, min};

pub fn and_not<const STACK: usize, const TASKS: usize, const NODES: usize>(
    left: &Bdd,
    right: &Bdd,
) -> Bdd {
    debug_assert!(left.node_count() * right.node_count() < usize::from(u16::MAX));
    let variable_count = max(left.variable_count(), right.variable_count());

    let mut result = Bdd::new_true_with_variables(variable_count);
    let mut result_is_empty = true;

    let mut task_cache: StaticTaskCache<TASKS> =
        StaticTaskCache::new(left.node_count(), right.node_count());
    let mut node_cache: StaticNodeCache<NODES> =
        StaticNodeCache::new(max(left.node_count(), right.node_count()));
    let mut stack: StaticStack<STACK> = StaticStack::new(variable_count);
    stack.push(
        left.root_pointer().into_u16(),
        right.root_pointer().into_u16(),
    );

    while !stack.is_empty() {
        let (l, r) = stack.peek();

        if task_cache.read(l, r).is_undefined() {
            let (l_var, r_var) = (left.var_of(l.into()), right.var_of(r.into()));
            let decision_variable = min(l_var, r_var);

            let (l_low, l_high) = if l_var != decision_variable {
                (l, l)
            } else {
                let (low, high) = left.pointers_of(l.into()).unpack();
                (low.into_u16(), high.into_u16())
            };

            let (r_low, r_high) = if r_var != decision_variable {
                (r, r)
            } else {
                let (low, high) = right.pointers_of(r.into()).unpack();
                (low.into_u16(), high.into_u16())
            };

            // Inlined implementation of the and_not logical table:
            // false & !??? = false
            // ??? & !true = false
            // true & !false = true
            // otherwise undefined...

            let low_result = if l_low.is_zero() || r_low.is_one() {
                PointerU16::ZERO
            } else if l_low.is_one() && r_low.is_zero() {
                PointerU16::ONE
            } else {
                task_cache.read(l_low, r_low)
            };
            let high_result = if l_high.is_zero() || r_high.is_one() {
                PointerU16::ZERO
            } else if l_high.is_one() && r_high.is_zero() {
                PointerU16::ONE
            } else {
                task_cache.read(l_high, r_high)
            };

            if !low_result.is_undefined() && !high_result.is_undefined() {
                if low_result.is_one() || high_result.is_one() {
                    result_is_empty = false;
                }

                let result = if low_result == high_result {
                    low_result
                } else {
                    let node = NodeU64::pack(decision_variable, low_result, high_result);
                    let saved = node_cache.read(node);
                    if saved.is_undefined() {
                        let pointer = result
                            .create_node(decision_variable, low_result.into(), high_result.into())
                            .into_u16();
                        node_cache.write(node, pointer);
                        pointer
                    } else {
                        saved
                    }
                };

                task_cache.write(l, r, result);
                stack.pop();
            } else {
                if low_result.is_undefined() {
                    stack.push(l_low, r_low);
                }
                if high_result.is_undefined() {
                    stack.push(l_high, r_high);
                }
            }
        } else {
            stack.pop();
        }
    }

    if result_is_empty {
        Bdd::new_false()
    } else {
        result
    }
}
