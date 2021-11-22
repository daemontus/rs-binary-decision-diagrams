use crate::{IntoIndex, FromIndex};

/// A unique integer reference to an existing node within some BDD.
///
/// The largest index is reserved as the `UNDEFINED` value, so the actual range is "only"
/// `2^64 - 1`. However, you can probably reasonably expect that this number will not exceed
/// `2^56` on any real computer in this century, so you can use the upper 8 bits for some
/// metadata if you want to. Note that this is not checked anywhere, so always make sure you
/// erase the metadata when interfacing with the BDD implementation (e.g. using a wrapper type).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct NodeIndex(u64);

impl NodeIndex {
    pub const UNDEFINED: NodeIndex = NodeIndex(u64::MAX);
    pub const ZERO: NodeIndex = NodeIndex(0);
    pub const ONE: NodeIndex = NodeIndex(1);

    #[inline]
    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    #[inline]
    pub fn is_one(&self) -> bool {
        *self == Self::ONE
    }

}

impl From<u64> for NodeIndex {
    fn from(value: u64) -> Self {
        NodeIndex(value)
    }
}

impl From<NodeIndex> for u64 {
    fn from(value: NodeIndex) -> Self {
        value.0
    }
}

impl IntoIndex for NodeIndex {
    fn into_index(self) -> usize {
        self.0.into_index()
    }
}

impl FromIndex for NodeIndex {
    fn from_index(index: usize) -> Self {
        NodeIndex(u64::from_index(index))
    }
}