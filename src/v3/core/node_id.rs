
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NodeId(u64);

impl NodeId {
    pub const ZERO: NodeId = NodeId(0);
    pub const ONE: NodeId = NodeId(1);
    pub const UNDEFINED: NodeId = NodeId(u64::MAX);

    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    pub fn is_one(&self) -> bool {
        *self == Self::ONE
    }

    pub fn is_terminal(&self) -> bool {
        self.is_zero() || self.is_one()
    }

    /// A utility wrapper around `NodeId::into` to avoid type inference that does not work
    /// for index values.
    pub fn into_usize(self) -> usize {
        self.into()
    }
}

/// A conversion from a 64-bit value to a `NodeId` is always correct, but of course, the resulting
/// `NodeId` may not be valid in the corresponding BDD if the value is out of bounds.
impl From<u64> for NodeId {
    fn from(value: u64) -> Self {
        NodeId(value)
    }
}

/// This conversion is also safe because we assume a 64-bit system.
impl From<usize> for NodeId {
    fn from(value: usize) -> Self {
        NodeId(value as u64)
    }
}

impl From<NodeId> for u64 {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

/// This unchecked conversion is safe on any 64-bit system, but can be problematic on 32-bit
/// systems. Right now, it is safe because we only support 64-bit systems.
impl From<NodeId> for usize {
    fn from(value: NodeId) -> Self {
        value.0 as usize
    }
}