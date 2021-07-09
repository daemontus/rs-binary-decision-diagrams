use std::ops::{Shl, BitOr};
use crate::{Pointer, SEED64, Bdd, Variable, PointerPair};
use std::cmp::{max, min};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct PointerU8(u8);

impl PointerU8 {

    fn zero() -> PointerU8 {
        PointerU8(0)
    }

    fn one() -> PointerU8 {
        PointerU8(0)
    }

    fn undef() -> PointerU8 {
        PointerU8(u8::MAX)
    }

    #[inline]
    fn as_bool(&self) -> Option<bool> {
        match self.0 {
            0 => Some(false),
            1 => Some(true),
            _ => None
        }
    }

    #[inline]
    fn from_bool(value: bool) -> PointerU8 {
        if value {
            PointerU8::one()
        } else {
            PointerU8::zero()
        }
    }

    #[inline]
    fn is_undef(&self) -> bool {
        self.0 == u8::MAX
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.0 == 1
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    fn is_terminal(&self) -> bool {
        self.0 <= 1
    }

}

struct FixedTaskCacheU8 {
    items: [(u8, u8); 256],
    values: [u32; 256],
    left_size: usize,
    right_size: usize,
    everything: Vec<u32>,
    marks: Vec<bool>,
}

impl FixedTaskCacheU8 {

    pub fn new(left_size: usize, right_size: usize) -> FixedTaskCacheU8 {
        FixedTaskCacheU8 {
            items: [(0, 0); 256],
            values: [0; 256],
            left_size, right_size,
            everything: vec![u32::MAX; left_size * right_size],
            marks: vec![false; left_size * right_size],
        }
    }

    #[inline]
    pub fn contains(&self, x: PointerU8, y: PointerU8) -> bool {
        debug_assert!(x.0 != 0 || y.0 != 0);
        let index = Self::hashed_index(x.0, y.0);
        let table_index = self.table_index(x.0, y.0);
        unsafe {
            self.items.get_unchecked(index) == &(x.0, y.0) || self.everything.get_unchecked(table_index) != &u32::MAX
        }
    }

    pub fn mark(&mut self, x: PointerU8, y: PointerU8) {
        let index = self.table_index(x.0, y.0);
        unsafe {
            let i = self.marks.get_unchecked_mut(index);
            *i = true;
        }
    }

    pub fn is_marked(&self, x: PointerU8, y: PointerU8) -> bool {
        let index = self.table_index(x.0, y.0);
        unsafe { *(self.marks.get_unchecked(index)) }
    }

    #[inline]
    pub fn get(&self, x: PointerU8, y: PointerU8) -> Pointer {
        debug_assert!(x.0 != 0 || y.0 != 0);
        let index = Self::hashed_index(x.0, y.0);
        let hashed_value = unsafe { self.items.get_unchecked(index) };
        let table_index = self.table_index(x.0, y.0);
        if hashed_value == &(x.0, y.0) {
            Pointer(*unsafe { self.values.get_unchecked(index) })
        } else {
            let result = unsafe { self.everything.get_unchecked(table_index) };
            // If result was not set, the value will be u32::MAX which is undef
            Pointer(*result)
        }
    }

    #[inline]
    pub fn insert(&mut self, x: PointerU8, y: PointerU8, result: Pointer) {
        debug_assert!(x.0 != 0 || y.0 != 0);
        let index = Self::hashed_index(x.0, y.0);
        let table_index = self.table_index(x.0, y.0);
        unsafe {
            let i = self.items.get_unchecked_mut(index);
            *i = (x.0, y.0);
            let j = self.values.get_unchecked_mut(index);
            *j = result.0;
            let k = self.everything.get_unchecked_mut(table_index);
            *k = result.0;
        }
    }

    #[inline]
    fn hashed_index(x: u8, y: u8) -> usize {
        (((x as usize).shl(8usize) + (y as usize)).wrapping_mul(SEED64 as usize) % 256)
    }

    fn table_index(&self, x: u8, y: u8) -> usize {
        (y as usize) * self.right_size + (x as usize)
    }
}

impl BitOr<PointerU8> for PointerU8 {
    type Output = PointerPair;

    fn bitor(self, rhs: PointerU8) -> Self::Output {
        Pointer::from(self).pair_with(rhs.into())
    }
}

impl From<Pointer> for PointerU8 {
    fn from(value: Pointer) -> Self {
        // Lossy conversion!!!
        PointerU8(value.0 as u8)
    }
}

impl From<PointerU8> for Pointer {
    fn from(value: PointerU8) -> Self {
        Pointer(u32::from(value.0))
    }
}

struct FixedNodeCacheU8 {
    items: [(Variable, PointerPair); 512],
    values: [u32; 512],
}

impl FixedNodeCacheU8 {

    pub fn new() -> FixedNodeCacheU8 {
        FixedNodeCacheU8 {
            items: [(Variable(u16::MAX), PointerPair::from(0)); 512],
            values: [0; 512],
        }
    }

    pub fn get(&self, var: Variable, pointers: PointerPair) -> Option<Pointer> {
        let index = Self::hashed_index(pointers.0);
        let x = unsafe { self.items.get_unchecked(index) };
        if x == &(var, pointers) {
            Some(Pointer(self.values[index]))
        } else {
            None
        }
    }

    pub fn insert(&mut self, var: Variable, pointers: PointerPair, result: Pointer) {
        let index = Self::hashed_index(pointers.0);
        unsafe {
            let i = self.items.get_unchecked_mut(index);
            *i = (var, pointers);
            let j = self.values.get_unchecked_mut(index);
            *j = result.0;
        }
    }

    #[inline]
    fn hashed_index(pointer_pair: u64) -> usize {
        (pointer_pair.wrapping_mul(SEED64) as usize) % 512
    }

}

struct StackU8 {
    index_after_top: usize,
    items: [Frame; 1024],
}

impl StackU8 {
    pub fn new() -> StackU8 {
        StackU8 {
            index_after_top: 0,
            items: [Frame::default(); 1024]
        }
    }

    pub fn peek(&self) -> Option<&Frame> {
        if self.index_after_top == 0 {
            None
        } else {
            Some(&self.items[self.index_after_top - 1])
        }
    }

    pub fn pop(&mut self) {
        self.index_after_top -= 1;
    }

    pub fn len(&self) -> usize {
        self.index_after_top
    }

    pub fn pop_and_get(&mut self) -> Frame {
        self.index_after_top -= 1;
        self.items[self.index_after_top].clone()
    }

    pub fn push(&mut self, frame: Frame) {
        if self.index_after_top == 512 {
            panic!("Overflow!");
        }
        self.items[self.index_after_top] = frame;
        self.index_after_top += 1;
    }

}

pub(crate) fn u8_apply<T>(left: &Bdd, right: &Bdd, lookup_table: T) -> Bdd where
    T: Fn(Option<bool>, Option<bool>) -> Option<bool> {
    debug_assert!(left.node_count() <= u8::MAX as usize && right.node_count() <= u8::MAX as usize);

    let variable_count = max(left.variable_count(), right.variable_count());
    let mut result = Bdd::new_true_with_variables(variable_count);

    let mut result_is_empty = true;
    let mut task_cache = FixedTaskCacheU8::new(left.node_count(), right.node_count());
    let mut node_cache = FixedNodeCacheU8::new();
    let mut stack = StackU8::new();
    //stack.push(left.root_pointer().into(), right.root_pointer().into());
    stack.push(Frame::new(left.root_pointer().into(), right.root_pointer().into()));

    loop {
        if stack.len() >= 4 {
            let frame1 = stack.pop_and_get();
            let frame2 = stack.pop_and_get();
            let frame3 = stack.pop_and_get();
            let frame4 = stack.pop_and_get();
            if frame4.is_expanded() {
                if !frame4.finalize(&mut task_cache, &mut node_cache, &mut result, &mut result_is_empty, &lookup_table) {
                    stack.push(frame4);
                }
            } else {
                frame4.expand(left, right, &task_cache, &mut stack);
            }
            if frame3.is_expanded() {
                if !frame3.finalize(&mut task_cache, &mut node_cache, &mut result, &mut result_is_empty, &lookup_table) {
                    stack.push(frame3);
                }
            } else {
                frame3.expand(left, right, &task_cache, &mut stack);
            }
            if frame2.is_expanded() {
                if !frame2.finalize(&mut task_cache, &mut node_cache, &mut result, &mut result_is_empty, &lookup_table) {
                    stack.push(frame2);
                }
            } else {
                frame2.expand(left, right, &task_cache, &mut stack);
            }
            if frame1.is_expanded() {
                if !frame1.finalize(&mut task_cache, &mut node_cache, &mut result, &mut result_is_empty, &lookup_table) {
                    stack.push(frame1);
                }
            } else {
                frame1.expand(left, right, &task_cache, &mut stack);
            }
        } else if stack.len() > 0 {
            let frame = stack.pop_and_get();
            if frame.is_expanded() {
                if !frame.finalize(&mut task_cache, &mut node_cache, &mut result, &mut result_is_empty, &lookup_table) {
                    stack.push(frame);
                }
            } else {
                frame.expand(left, right, &task_cache, &mut stack);
            }
        } else {
            break;
        }
    }
    /*
    while let Some((l, r)) = stack.peek() {
        let (l_var, r_var) = (left.var_of(l.into()), right.var_of(r.into()));
        let decision_var = min(l_var, r_var);

        let (l_low, l_high) = if l_var != decision_var {
            (l, l)
        } else {
            let (x, y) = left.pointers_of(l.into()).unpack();
            (x.into(), y.into())
        };
        let (r_low, r_high) = if r_var != decision_var {
            (r, r)
        } else {
            let (x, y) = right.pointers_of(r.into()).unpack();
            (x.into(), y.into())
        };

        let low_result = if let Some(value) = lookup_table(l_low.as_bool(), r_low.as_bool()) {
            Pointer::from_bool(value)
        } else {
            task_cache.get(l_low, r_low)
        };

        let high_result = if let Some(value) = lookup_table(l_high.as_bool(), r_high.as_bool()) {
            Pointer::from_bool(value)
        } else {
            task_cache.get(l_high, r_high)
        };

        if !low_result.is_undef() && !high_result.is_undef() {
            if low_result.is_one() || high_result.is_one() {
                result_is_empty = false
            }

            if low_result == high_result {
                task_cache.insert(l, r, low_result);
            } else {
                let result_pair = low_result | high_result;
                if let Some(pointer) = node_cache.get(decision_var, result_pair) {
                    task_cache.insert(l, r, pointer);
                } else {
                    result.push_node(decision_var, result_pair);
                    task_cache.insert(l, r, result.root_pointer());
                    node_cache.insert(decision_var, result_pair, result.root_pointer());
                }
            }

            stack.pop();
        } else {
            if low_result.is_undef() {
                stack.push(l_low, r_low);
            }
            if high_result.is_undef() {
                stack.push(l_high, r_high);
            }
        }
    }*/

    if result_is_empty {
        Bdd::new_false()
    } else {
        result
    }
}

#[derive(Clone, Copy)]
struct Frame {
    task: (PointerU8, PointerU8),
    var: Variable,
    low: (PointerU8, PointerU8),
    high: (PointerU8, PointerU8),
}

impl Default for Frame {
    fn default() -> Self {
        Frame::new(PointerU8::undef(), PointerU8::undef())
    }
}

impl Frame {

    fn new(l: PointerU8, r: PointerU8) -> Frame {
        Frame {
            task: (l, r),
            var: Variable(u16::MAX),
            low: (PointerU8::undef(), PointerU8::undef()),
            high: (PointerU8::undef(), PointerU8::undef()),
        }
    }

    #[inline]
    fn is_expanded(&self) -> bool {
        self.var.0 != u16::MAX
    }

    #[inline]
    fn finalize<T: Fn(Option<bool>, Option<bool>) -> Option<bool>>(
        &self,
        task_cache: &mut FixedTaskCacheU8,
        node_cache: &mut FixedNodeCacheU8,
        result: &mut Bdd,
        result_is_empty: &mut bool,
        lookup_table: &T
    ) -> bool {
        let (l_low, r_low) = self.low;
        let low_result = if let Some(value) = lookup_table(l_low.as_bool(), r_low.as_bool()) {
            Pointer::from_bool(value)
        } else {
            task_cache.get(l_low, r_low)
        };

        let (l_high, r_high) = self.high;
        let high_result = if let Some(value) = lookup_table(l_high.as_bool(), r_high.as_bool()) {
            Pointer::from_bool(value)
        } else {
            task_cache.get(l_high, r_high)
        };

        if !low_result.is_undef() && !high_result.is_undef() {
            if low_result.is_one() || high_result.is_one() {
                *result_is_empty = false
            }

            let (l,r) = self.task;
            let decision_var = self.var;

            if low_result == high_result {
                task_cache.insert(l, r, low_result);
            } else {
                let result_pair = low_result | high_result;
                if let Some(pointer) = node_cache.get(decision_var, result_pair) {
                    task_cache.insert(l, r, pointer);
                } else {
                    result.push_node(decision_var, result_pair);
                    task_cache.insert(l, r, result.root_pointer());
                    node_cache.insert(decision_var, result_pair, result.root_pointer());
                }
            }
            true
        } else {
            false
        }
    }

    #[inline]
    fn expand(mut self, left: &Bdd, right: &Bdd, task_cache: &FixedTaskCacheU8, stack: &mut StackU8) {
        let (l, r) = self.task;
        let (l_var, r_var) = (left.var_of(l.into()), right.var_of(r.into()));
        let decision_var = min(l_var, r_var);
        self.var = decision_var;

        let (l_low, l_high) = if l_var != decision_var {
            (l, l)
        } else {
            let (x, y) = left.pointers_of(l.into()).unpack();
            (x.into(), y.into())
        };
        let (r_low, r_high) = if r_var != decision_var {
            (r, r)
        } else {
            let (x, y) = right.pointers_of(r.into()).unpack();
            (x.into(), y.into())
        };

        self.low = (l_low, r_low);
        self.high = (l_high, r_high);

        stack.push(self);

        if !(l_low.is_terminal() && r_low.is_terminal()) && !task_cache.contains(l_low, r_low) {
            stack.push(Frame::new(l_low, r_low));
        }
        if !(l_high.is_terminal() && r_high.is_terminal()) && !task_cache.contains(l_high, r_high) {
            stack.push(Frame::new(l_high, r_high));
        }
    }

}
/*
#[inline]
fn loop_body<T>(
    left: &Bdd,
    right: &Bdd,
    l: PointerU8,
    r: PointerU8,
    task_cache: &mut FixedTaskCacheU8,
    node_cache: &mut FixedNodeCacheU8,
    lookup_table: &T,
    result: &mut Bdd,
    result_is_empty: &mut bool,
    stack: &mut StackU8,
) where T: Fn(Option<bool>, Option<bool>) -> Option<bool> {
    let (l_var, r_var) = (left.var_of(l.into()), right.var_of(r.into()));
    let decision_var = min(l_var, r_var);

    let (l_low, l_high) = if l_var != decision_var {
        (l, l)
    } else {
        let (x, y) = left.pointers_of(l.into()).unpack();
        (x.into(), y.into())
    };
    let (r_low, r_high) = if r_var != decision_var {
        (r, r)
    } else {
        let (x, y) = right.pointers_of(r.into()).unpack();
        (x.into(), y.into())
    };

    let low_result = if let Some(value) = lookup_table(l_low.as_bool(), r_low.as_bool()) {
        Pointer::from_bool(value)
    } else {
        task_cache.get(l_low, r_low)
    };

    let high_result = if let Some(value) = lookup_table(l_high.as_bool(), r_high.as_bool()) {
        Pointer::from_bool(value)
    } else {
        task_cache.get(l_high, r_high)
    };

    if !low_result.is_undef() && !high_result.is_undef() {
        if low_result.is_one() || high_result.is_one() {
            *result_is_empty = false
        }

        if low_result == high_result {
            task_cache.insert(l, r, low_result);
        } else {
            let result_pair = low_result | high_result;
            if let Some(pointer) = node_cache.get(decision_var, result_pair) {
                task_cache.insert(l, r, pointer);
            } else {
                result.push_node(decision_var, result_pair);
                task_cache.insert(l, r, result.root_pointer());
                node_cache.insert(decision_var, result_pair, result.root_pointer());
            }
        }

    } else {
        stack.push(l, r);
        if low_result.is_undef() {
            stack.push(l_low, r_low);
        }
        if high_result.is_undef() {
            stack.push(l_high, r_high);
            task_cache.mark(l_high, r_high);
        }
    }
}
*/
#[cfg(test)]
mod tests {
    use crate::Bdd;
    use std::convert::TryFrom;

    #[test]
    fn test() {
        let mut benchmarks = Vec::new();
        for file in std::fs::read_dir("./bench_inputs/itgr").unwrap() {
            let file = file.unwrap();
            let path = file.path();
            let file_name = path.file_name().unwrap().to_str().unwrap();
            if file_name.ends_with(".and_not.left.bdd") {
                let bench_name = &file_name[..(file_name.len() - 17)];
                benchmarks.push(bench_name.to_string());
            }
        }
        // Actually do the benchmarks in some sensible order.
        benchmarks.sort_by_cached_key(|name| {
            let mut split = name.split(".");
            split.next();
            let size = split.next().unwrap();
            size.parse::<usize>().unwrap()
        });

        for benchmark in &benchmarks {
            let mut split = benchmark.split(".");
            split.next();
            let size = split.next().unwrap();
            let node_count = size.parse::<usize>().unwrap();
            if node_count <= 256 {
                let left_path = format!("./bench_inputs/itgr/{}.and_not.left.bdd", benchmark);
                let left = Bdd::try_from(std::fs::read_to_string(&left_path).unwrap().as_str()).unwrap();
                let right_path = format!("./bench_inputs/itgr/{}.and_not.right.bdd", benchmark);
                let right = Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
                let result = left.and_not(&right);
                println!("{}: {}", benchmark, result.node_count());
            }
        }
    }

}