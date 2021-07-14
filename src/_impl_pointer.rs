use crate::{Pointer, PointerPair};
use std::ops::{BitOr, Shl, Shr};

impl Pointer {
    #[inline]
    pub fn zero() -> Pointer {
        Pointer(0)
    }

    #[inline]
    pub fn one() -> Pointer {
        Pointer(1)
    }

    #[inline]
    pub fn undef() -> Pointer {
        Pointer(u32::MAX)
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.0 < 2
    }

    #[inline]
    pub fn is_one(&self) -> bool {
        self.0 == 1
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn pair_with(self, other: Pointer) -> PointerPair {
        PointerPair(u64::from(self.0).shl(32) + u64::from(other.0))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.0 {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn from_bool(value: bool) -> Pointer {
        if value {
            Pointer::one()
        } else {
            Pointer::zero()
        }
    }

    pub fn is_undef(&self) -> bool {
        self.0 == u32::MAX
    }
}

impl PointerPair {
    #[inline]
    pub fn unpack(self) -> (Pointer, Pointer) {
        (Pointer(self.0.shr(32) as u32), Pointer(self.0 as u32))
    }
}

impl BitOr<Pointer> for Pointer {
    type Output = PointerPair;

    fn bitor(self, rhs: Pointer) -> Self::Output {
        self.pair_with(rhs)
    }
}

impl From<u32> for Pointer {
    fn from(value: u32) -> Self {
        Pointer(value)
    }
}

impl From<u64> for PointerPair {
    fn from(value: u64) -> Self {
        PointerPair(value)
    }
}
