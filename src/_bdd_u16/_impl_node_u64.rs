use crate::Variable;
use crate::_bdd_u16::{NodeU64, PointerU16};
use std::ops::Shl;

impl NodeU64 {
    pub const UNDEFINED: NodeU64 = NodeU64(u64::MAX);

    /// Pack node data into the `NodeU64` struct.
    pub fn pack(variable: Variable, low: PointerU16, high: PointerU16) -> NodeU64 {
        NodeU64((variable.0 as u64).shl(32) + (low.0 as u64).shl(16) + (high.0 as u64))
    }
}
