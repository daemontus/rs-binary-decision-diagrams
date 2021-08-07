use crate::machine::{NodeId, VariableId};

/// A compact representation of a BDD node, packed into 16 bytes.
///
/// First 8-byte value is the low link id, second 8-byte value is the high link id combined
/// with the variable id. The reasoning behind this is that during normal traversal, low link
/// is needed first, while high link is usually explored second, hence can require
/// more instructions to obtain.
///
/// The nice thing about this is that it is easy to align at 8-byte boundaries (which CPUs like),
/// but also has some empty bits that can be used for magic if needed.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BddNode(u64, u64);

impl BddNode {
    pub const ZERO: BddNode =
        unsafe { BddNode::pack_unchecked(VariableId::UNDEFINED, NodeId::ZERO, NodeId::ZERO) };

    pub const ONE: BddNode =
        unsafe { BddNode::pack_unchecked(VariableId::UNDEFINED, NodeId::ONE, NodeId::ONE) };

    /// An unsafe version of `BddNode::pack` which does not check whether arguments are valid.
    ///
    /// You can use this function to create `BddNode::ZERO` and `BddNode::ONE` nodes if you
    /// really really have to, but try to use the predefined constants when possible.
    pub const unsafe fn pack_unchecked(variable: VariableId, low: NodeId, high: NodeId) -> BddNode {
        BddNode(
            low.into_u64(),
            high.into_u64() | (variable.into_u64() << 48),
        )
    }

    /// Pack given `variable` together with `low` and `high` links into a single node.
    ///
    /// Arguments must not be undefined and `low != high`. This means you can't create
    /// a terminal node with this function. For that, please use predefined constants
    /// `BddNode::ZERO` and `BddNode::ONE`. However, you should not need to create terminal
    /// nodes under normal circumstances anyway.
    pub fn try_pack(variable: VariableId, low: NodeId, high: NodeId) -> Option<BddNode> {
        let links_invalid = low.is_undefined() || high.is_undefined() || low == high;
        if variable.is_undefined() || links_invalid {
            None
        } else {
            Some(unsafe { BddNode::pack_unchecked(variable, low, high) })
        }
    }

    /// Unpack a `BddNode` into a decision variable, low link and high link.
    pub fn unpack(self) -> (VariableId, NodeId, NodeId) {
        unsafe {
            // Operations are safe due to the way values are packed in the u64 integers.
            let var = VariableId::from_u64(self.1 >> 48);
            let low = NodeId::from_u64(self.0);
            let high = NodeId::from_u48(self.1);
            (var, low, high)
        }
    }

    /// Read the decision variable.
    ///
    /// WARNING: The result can be undefined if called on a terminal node!
    #[inline]
    pub fn variable(&self) -> VariableId {
        unsafe { VariableId::from_u64(self.1 >> 48) }
    }

    /// Read the low link.
    #[inline]
    pub fn low_link(&self) -> NodeId {
        unsafe { NodeId::from_u64(self.0) }
    }

    /// Read the high link.
    #[inline]
    pub fn high_link(&self) -> NodeId {
        NodeId::from_u48(self.1)
    }

    /// Read the low and high links.
    #[inline]
    pub fn links(&self) -> (NodeId, NodeId) {
        (self.low_link(), self.high_link())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{NodeId, VariableId};
    use super::BddNode;

    const ID_5: NodeId = NodeId::from_u48(5);
    const ID_62: NodeId = NodeId::from_u48(62);
    const VAR_13: VariableId = VariableId::from_u16(13);

    #[test]
    fn bdd_node_pack_unpack() {
        let node = BddNode::try_pack(VAR_13, ID_5, ID_62).unwrap();
        assert_eq!(VAR_13, node.variable());
        assert_eq!(ID_5, node.low_link());
        assert_eq!(ID_62, node.high_link());
        assert_eq!((ID_5, ID_62), node.links());
        assert_eq!((VAR_13, ID_5, ID_62), node.unpack());
    }

    #[test]
    fn bdd_node_invalid_args() {
        assert!(BddNode::try_pack(VariableId::UNDEFINED, ID_5, ID_62).is_none());
        assert!(BddNode::try_pack(VAR_13, NodeId::UNDEFINED, ID_62).is_none());
        assert!(BddNode::try_pack(VAR_13, ID_5, NodeId::UNDEFINED).is_none());
        assert!(BddNode::try_pack(VAR_13, ID_5, ID_5).is_none());
    }
}
