/// Unique identifier of one Boolean decision variable in a BDD.
///
/// It ranges from `0` to `u16::MAX - 1`, with `u16::MAX` reserved as special *undefined* value.
/// The main purpose of this undefined value is to be able to also express the "number of variable
/// ids"  using a `u16` integer.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct VariableId(u16);

impl VariableId {
    pub const UNDEFINED: VariableId = VariableId(u16::MAX);

    /// **(internal)** A const version of `VariableId::from(u16)`.
    pub(crate) const fn from_u16(value: u16) -> VariableId {
        VariableId(value)
    }

    /// **(internal)** Unchecked conversion from `u64` to `VariableId`.
    pub(crate) unsafe fn from_u64(value: u64) -> VariableId {
        debug_assert!(value <= u64::from(u16::MAX));
        VariableId(value as u16)
    }

    /// **(internal)** Convert this `VariableId` to `u64`.
    pub(crate) const fn into_u64(self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }
}

impl From<u16> for VariableId {
    fn from(value: u16) -> Self {
        VariableId::from_u16(value)
    }
}

impl From<VariableId> for u16 {
    fn from(value: VariableId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::VariableId;

    #[test]
    fn variable_id_valid_conversions() {
        let ten = VariableId(10);
        let undef = VariableId::UNDEFINED;

        assert!(VariableId::from(u16::MAX).is_undefined());
        assert_eq!(ten, VariableId::from(u16::from(ten)));
        assert_eq!(undef, VariableId::from(u16::from(undef)));
        unsafe {
            assert_eq!(ten, VariableId::from_u64(ten.into_u64()));
            assert_eq!(undef, VariableId::from_u64(undef.into_u64()));
        }
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn variable_id_invalid_conversions() {
        unsafe {
            assert!(VariableId::from_u64(1 << 32).is_undefined());
        }
    }
}
