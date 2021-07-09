
mod _impl_apply;
mod _impl_node_u64;
mod _impl_pointer_u16;
mod _impl_static_task_cache;
mod _impl_static_node_cache;
mod _impl_static_stack;

pub use _impl_apply::and_not;

/// A 2-byte version of the standard pointer. It is not exactly easier to work with in terms of
/// raw instructions, but it saves a lot of valuable space in L1/L2 cache that would be
/// otherwise wasted...
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct PointerU16(u16);

/// A `Bdd` node based on `u16` pointers encoded into an `u64` value. The reason for this encoding
/// is that hashing and comparing such nodes will be easier
/// (since it can be done in one instruction).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct NodeU64(u64);


/// Task cache maps pairs of "input" pointers to the resulting "output" pointer.
///
/// The cache is "complete" in the sense that it does not lose information during collisions,
/// as it uses a complete "backing table". The table is a stack-allocated array, with the capacity
/// being the generic argument of the cache.
///
/// TODO: Try to also insert a small hashed "pre-cache" for fast access to commonly used items.
struct StaticTaskCache<const N: usize> {
    dimension_x: usize,
    dimension_y: usize,
    table: [PointerU16; N],
}

/// A node cache maps "output" `Bdd` nodes to "output" pointers.
///
/// The cache is "incomplete" in the sense that it can lose information upon collision.
/// However, in our experience this is quite rare and does not lead to significant blowup
/// in `Bdd` size.
struct StaticNodeCache<const N: usize> {
    keys: [NodeU64; N],
    values: [PointerU16; N],
}

struct StaticStack<const N: usize> {
    index_after_last: usize,
    items: [(PointerU16, PointerU16); N],
}