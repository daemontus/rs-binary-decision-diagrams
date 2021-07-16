use crate::v2::{Bdd, NodeId};
use std::cmp::max;
use std::ops::BitXor;

/// **(internal)** A general `apply` algorithm suitable for any `Bdd`.
mod u48;

/// **(internal)** An `apply` algorithm designed for `Bdd` with up to `2^32 - 1`
/// nodes. The main improvement is lower memory footprint and hence better caching.
///
/// The performance impact of this is not very drastic, but it is a nice, consistent
/// 10-15% improvement, so why not do it.
mod u32;

impl Bdd {
    /// A logical conjunction of two `Bdd` objects.
    pub fn and(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.and(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_and(other)
            } else {
                self._u48_and(other)
            }
        }
    }

    /// A logical disjunction of two `Bdd` objects.
    pub fn or(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.or(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_or(other)
            } else {
                self._u48_or(other)
            }
        }
    }

    /// A logical implication of two `Bdd` objects.
    pub fn imp(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.inv_imp(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_imp(other)
            } else {
                self._u48_imp(other)
            }
        }
    }

    /// **(internal)** A mirrored implication operation.
    fn inv_imp(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.imp(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_inv_imp(other)
            } else {
                self._u48_inv_imp(other)
            }
        }
    }

    /// A logical equivalence of two `Bdd` objects.
    pub fn iff(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.iff(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_iff(other)
            } else {
                self._u48_iff(other)
            }
        }
    }

    /// A logical exclusive disjunction of two `Bdd` objects.
    pub fn xor(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.xor(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_xor(other)
            } else {
                self._u48_xor(other)
            }
        }
    }

    /// A logical conjunction with a negated send argument of two `Bdd` objects.
    ///
    /// This method is used for set difference when using `Bdd` as a set representation.
    /// That is why it warrants a separate method.
    pub fn and_not(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.not_and(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_and_not(other)
            } else {
                self._u48_and_not(other)
            }
        }
    }

    /// **(internal)** A mirrored `and_not` operation.
    fn not_and(&self, other: &Bdd) -> Bdd {
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.and_not(self)
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                self._u32_not_and(other)
            } else {
                self._u48_not_and(other)
            }
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
        let self_nodes = self.node_count() as u64;
        let other_nodes = other.node_count() as u64;
        if other_nodes > self_nodes {
            other.binary_operation(self, |l, r| table(r, l))
        } else {
            if self_nodes < u32::MAX_LEFT_SIZE && other_nodes < u32::MAX_RIGHT_SIZE {
                u32::_u32_apply(self, other, table)
            } else {
                u48::_u48_apply(self, other, table)
            }
        }
    }
}
