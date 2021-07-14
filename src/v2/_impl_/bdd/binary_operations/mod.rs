use crate::v2::{Bdd, NodeId};
use crate::v2::_impl_::bdd::binary_operations::u48::apply;
use std::cmp::max;

/// A general `apply` algorithm suitable for any `Bdd`.
pub mod u48;

impl Bdd {

    pub fn and_not(&self, other: &Bdd) -> Bdd {
        /*apply(self, other, |l, r| {
            if l.is_zero() || r.is_one() {
                NodeId::ZERO
            } else if l.is_one() && r.is_zero() {
                NodeId::ONE
            } else {
                NodeId::UNDEFINED
            }
        })*/
        self.and_not_u48(other)
    }

}