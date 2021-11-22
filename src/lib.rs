// Force the unsafe code in unsafe functions to be properly annotated as such.
#![warn(unsafe_op_in_unsafe_fn)]

#[macro_use]
extern crate static_assertions;

// Ensures that this library only works on 64-bit systems.
// In the future, we should consider extending support to 32-bit apps or 16-bit apps,
// but that is a fun project for someone with more free time.
//
// For the conversion, please use exclusively the `IntoIndex` trait defined below,
// such that in the future, we can track where the conversions happen.
assert_eq_size!(usize, u64);

trait IntoIndex {
    fn into_index(self) -> usize;
}

trait FromIndex {
    fn from_index(index: usize) -> Self;
}

impl IntoIndex for u16 {
    fn into_index(self) -> usize {
        usize::from(self)
    }
}

impl IntoIndex for u32 {
    fn into_index(self) -> usize {
        self as usize
    }
}

impl IntoIndex for u64 {
    fn into_index(self) -> usize {
        self as usize
    }
}

impl FromIndex for u64 {
    fn from_index(index: usize) -> Self {
        index as u64
    }
}

pub mod v2;
pub mod v3;
pub mod v4;

pub mod machine;

pub mod perf_testing;