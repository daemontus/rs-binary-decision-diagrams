use crate::v3::core::node_id::NodeId;
use crate::v3::core::variable_id::VariableId;

/// A packed representation of a BDD node. The reason we have this struct is that
/// in the future, we would like to test more compact representations (i.e. 32-bit variable,
/// 48-bit address) and we need a clear separation between node contents and node representation.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct PackedBddNode {
    variable: u64, // But will always only hold 32-bit values.
    low_link: u64,
    high_link: u64,
}

impl PackedBddNode {

    pub fn pack(variable: VariableId, low_link: NodeId, high_link: NodeId) -> PackedBddNode {
        PackedBddNode {
            variable: variable.into(),
            low_link: low_link.into(),
            high_link: high_link.into(),
        }
    }

    pub fn unpack(self) -> (VariableId, NodeId, NodeId) {
        unsafe {
            (VariableId::from_u64_unchecked(self.variable), self.low_link.into(), self.high_link.into())
        }
    }

    pub fn get_variable(&self) -> VariableId {
        unsafe { VariableId::from_u64_unchecked(self.variable) }
    }

    pub fn get_low_link(&self) -> NodeId {
        self.low_link.into()
    }

    pub fn get_high_link(&self) -> NodeId {
        self.high_link.into()
    }

}