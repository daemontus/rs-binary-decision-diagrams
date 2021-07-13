
pub mod v2;

mod _impl_bdd_utils;
mod _impl_pointer;

mod _impl_variable;
mod _impl_apply;

/// Static task cache is designed to hold information about internal BDD operations that
/// were completed and produced a value. It is a perfect mapping, i.e. it will never lose
/// values that are inserted into it.
mod static_task_cache;

/// A very optimized version of the apply algorithm for small BDDs with <256 nodes.
///
/// Does not allocate anything and uses compact memory structures.
mod _impl_u8;

/// Contains a faster implementation of `Bdd` operations for `Bdds` where pointers fit into `u16`.
pub(crate) mod _bdd_u16;
pub mod _bdd_u32;

const SEED64: u64 = 0x51_7c_c1_b7_27_22_0a_95;
const SEED32: u32 = 0x9e_37_79_b9;
const SEED16: u16 = SEED32 as u16;

pub mod function;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Variable(u16);

// Private types used for pointing to a single BDD node or a low/high pair of nodes.
// Value u32::MAX is reserved as an undefined value.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Pointer(u32);
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct PointerPair(u64);

// The reason for separating the variables from pointers is to make the
// representation as compact as possible so that we can fit as much as possible
// into L1/L2 cache. Also, some algorithms don't actually need to check both
// variable and pointers so these will benefit from better alignment.

#[derive(Clone)]
pub struct Bdd {
    //node_variables: Vec<Variable>,
    //node_pointers: Vec<PointerPair>,
    nodes: Vec<(u32, u16, Variable, PointerPair)>
}

