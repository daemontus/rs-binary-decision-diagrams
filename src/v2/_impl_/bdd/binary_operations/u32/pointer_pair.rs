use crate::v2::NodeId;
use super::PointerPair;

impl PointerPair {
    pub const RESULT_MASK: u64 = 1 << 63;
    const LEFT_POINTER_MASK: u64 = 0xffff_ffff;

    #[inline]
    pub fn pack(left: NodeId, right: NodeId) -> PointerPair {
        debug_assert!(left.0 < super::MAX_LEFT_SIZE);
        debug_assert!(left.0 < super::MAX_RIGHT_SIZE);
        // Left pointer goes into the "lower" bits.
        PointerPair((right.0 << 32) | left.0)
    }

    #[inline]
    pub fn from_result(result: NodeId) -> PointerPair {
        PointerPair(result.0 | Self::RESULT_MASK)
    }

    #[inline]
    pub fn unpack(self) -> (NodeId, NodeId) {
        (NodeId(self.0 & Self::LEFT_POINTER_MASK), NodeId(self.0 >> 32))
    }

    #[inline]
    pub fn is_result(&self) -> bool {
        self.0 & Self::RESULT_MASK != 0
    }

    #[inline]
    pub fn into_result(self) -> NodeId {
        debug_assert!(self.is_result());
        NodeId(self.0 ^ Self::RESULT_MASK)
    }

}

impl From<u64> for PointerPair {
    fn from(value: u64) -> Self {
        PointerPair(value)
    }
}

impl From<PointerPair> for u64 {
    fn from(value: PointerPair) -> Self {
        value.0
    }
}
