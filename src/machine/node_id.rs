use std::convert::TryFrom;

/// A unique identifier of a node in a BDD.
///
/// Node ids range from `0` to `2^48 - 1` with three special values:
///  - `0` and `1` are reserved as terminal ids.
///  - `2^48 - 1` is reserved as *undefined* value, but in general anything above `2^48`
///  should be considered as invalid node id.
///
/// The reason for limiting the range of the id to `2^48` is the ability to use the
/// additional bits in other data structures to pack extra useful data together with the id.
/// However, this should not be done directly using a `NodeId` but rather using other
/// appropriate type-safe wrappers.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NodeId(u64);

impl NodeId {
    /// Id of a *zero* terminal node.
    pub const ZERO: NodeId = NodeId(0);

    /// Id of a *one* terminal node.
    pub const ONE: NodeId = NodeId(1);

    /// Undefined id.
    pub const UNDEFINED: NodeId = NodeId((1 << 48) - 1);

    /// **(internal)** A mask of bits that are used in a valid `NodeId`.
    ///
    /// Used to quickly extract a `NodeId` from an integer with additional packed data.
    const BIT_MASK: u64 = (1 << 48) - 1;

    #[inline]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    #[inline]
    pub fn is_one(&self) -> bool {
        *self == Self::ONE
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.is_zero() || self.is_one()
    }

    /// True if this node id represents an undefined value.
    #[inline]
    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

    /// **(internal)** An explicit conversion into `u64`.
    ///
    /// It is a bit nicer to use than `u64::from(id)`.
    pub(crate) const fn into_u64(self) -> u64 {
        self.0
    }

    /// **(internal)** Unchecked conversion from `u64` to `NodeId`.
    ///
    /// The `u64` must be a valid `NodeId`. We do not truncate or wrap the data in any way.
    pub(crate) unsafe fn from_u64(value: u64) -> NodeId {
        debug_assert!(value < (1 << 48));
        NodeId(value)
    }

    /// **(internal)** Unchecked conversion from `NodeId` to `usize`.
    ///
    /// This operation is safe on 64-bit platforms, but *may* be overflow when `usize` is 32
    /// (or god forbid 16) bits.
    pub(crate) unsafe fn into_usize(self) -> usize {
        debug_assert!(usize::try_from(self.0).is_ok());
        self.0 as usize
    }

    /// **(internal)** Extract 48 least significant bits from a 64-bit value
    /// and interpret them as a `NodeId`.
    ///
    /// The difference between this and [NodeId::from_u64] is that this method
    /// will actually truncate the value to its 48 least significant bits and can be therefore
    /// used in situations where the number also contains additional data. The advantage of this
    /// is that the result is always a valid `NodeId` regardless of input.
    pub(crate) const fn from_u48(value: u64) -> NodeId {
        NodeId(value & Self::BIT_MASK)
    }
}

impl From<NodeId> for u64 {
    fn from(value: NodeId) -> Self {
        value.into_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::NodeId;

    #[test]
    fn node_id_basic_properties() {
        assert!(NodeId(0).is_zero());
        assert!(NodeId(1).is_one());
        assert!(NodeId(0).is_terminal() && NodeId(1).is_terminal());
        assert!(!NodeId(2).is_terminal() && !NodeId(2).is_undefined());
        assert!(NodeId(NodeId::BIT_MASK).is_undefined());
    }

    #[test]
    fn node_id_valid_conversions() {
        let five = NodeId(5);
        assert_eq!(five, NodeId::from_u48(u64::from(five)));
        assert_eq!(five, NodeId::from_u48((1 << 50) | 5));
        unsafe {
            assert_eq!(five, NodeId::from_u64(u64::from(five)));
            assert_eq!(five, NodeId::from_u48(five.into_usize() as u64));
        }
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn node_id_invalid_conversions() {
        unsafe {
            assert!(!NodeId::from_u64((1 << 50) | 5).is_terminal());
        }
    }
}
