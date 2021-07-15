use crate::v2::{Bdd, NodeId};
use std::cmp::max;
use std::ops::BitXor;

/// **(internal)** A general `apply` algorithm suitable for any `Bdd`.
mod u48;

/// **(internal)** An `apply` algorithm designed for `Bdd` with up to `2^32 - 1`
/// nodes. The main improvement is lower memory footprint.
///
/// The performance impact of this is not very drastic, but it is a nice, consistent
/// 10-15% improvement, so why not do it.
mod u32;

impl Bdd {
    /// A logical conjunction of two `Bdd` objects.
    pub fn and(&self, other: &Bdd) -> Bdd {
        self._u48_and(other) // Symmetric operation
    }

    /// A logical disjunction of two `Bdd` objects.
    pub fn or(&self, other: &Bdd) -> Bdd {
        self._u48_or(other) // Symmetric operation
    }

    /// A logical implication of two `Bdd` objects.
    pub fn imp(&self, other: &Bdd) -> Bdd {
        if other.node_count() > self.node_count() {
            other._u48_inv_imp(self)
        } else {
            self._u48_imp(other)
        }
    }

    /// A logical equivalence of two `Bdd` objects.
    pub fn iff(&self, other: &Bdd) -> Bdd {
        self._u48_iff(other) // Symmetric operation
    }

    /// A logical exclusive disjunction of two `Bdd` objects.
    pub fn xor(&self, other: &Bdd) -> Bdd {
        self._u48_xor(other) // Symmetric operation
    }

    /// A logical conjunction with a negated send argument of two `Bdd` objects.
    ///
    /// This method is used for set difference when using `Bdd` as a set representation.
    /// That is why it warrants a separate method.
    pub fn and_not(&self, other: &Bdd) -> Bdd {
        if other.node_count() > self.node_count() {
            other._u48_not_and(other)
        } else {
            self._u48_and_not(other)
        }
    }

    /// A general binary operation on two `Bdd` objects. The user provides a
    /// lookup `TABLE` which implements the logical operation.
    ///
    /// Given two terminal nodes, the table must return a terminal node (either `NodeId::ZERO`
    /// or `NodeId::ONE`). If the function allows it, the lookup table can return a terminal
    /// node even when one of the arguments is not terminal (for example, conjunction is false
    /// even if one argument is false).
    pub fn binary_operation<TABLE>(&self, other: &Bdd, table: TABLE) -> Bdd
    where
        TABLE: Fn(NodeId, NodeId) -> NodeId,
    {
        if other.node_count() > self.node_count() {
            other.binary_operation(self, |l, r| table(r, l))
        } else {
            u48::_u48_apply(self, other, table)
        }
    }
}
