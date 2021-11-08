// Force the unsafe code in unsafe functions to be properly annotated as such.
#![warn(unsafe_op_in_unsafe_fn)]

#[macro_use]
extern crate static_assertions;

// Ensures that this library only works on 64-bit systems.
// In the future, we should consider extending support to 32-bit apps or 16-bit apps,
// but that is a fun project for someone with more free time.
assert_eq_size!(usize, u64);

pub mod v2;
pub mod v3;

pub mod machine;

pub mod perf_testing;