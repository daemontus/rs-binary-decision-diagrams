use crate::_bdd_u16::PointerU16;
use crate::{Bdd, Pointer, Variable};

impl PointerU16 {
    pub const ZERO: PointerU16 = PointerU16(0);
    pub const ONE: PointerU16 = PointerU16(1);
    pub const UNDEFINED: PointerU16 = PointerU16(u16::MAX);

    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == u16::MAX
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_one(&self) -> bool {
        self.0 == 1
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.0 < 2
    }
}

/// A safe conversion from 16-bit to 32-bit pointer.
impl From<PointerU16> for Pointer {
    fn from(value: PointerU16) -> Self {
        Pointer(u32::from(value.0))
    }
}

/// A safe conversion from 16-bit pointer to usize.
impl Into<usize> for PointerU16 {
    fn into(self) -> usize {
        usize::from(self.0)
    }
}

impl Pointer {
    /// Convert this pointer to its `u16` version. The pointer must not be undefined.
    ///
    /// The validity of the conversion is only checked in debug mode!
    #[inline]
    pub(super) fn into_u16(self) -> PointerU16 {
        debug_assert!(self.0 < u32::from(u16::MAX));
        PointerU16(self.0 as u16)
    }
}
