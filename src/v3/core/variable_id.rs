
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VariableId(u32);

impl VariableId {
    pub const UNDEFINED: VariableId = VariableId(u32::MAX);

    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

    /// Unchecked conversion from a 64-bit value. The conversion may lose information if the
    /// value does not fit into 32-bits.
    ///
    /// Also, note that an `undefined`/`max` 32-bit value is not `undefined` in 64-bits,
    /// so semantics of undefined values may break.
    pub unsafe fn from_u64_unchecked(value: u64) -> VariableId {
        VariableId(value as u32)
    }

}

/// A conversion from a 32-bit value to a `VariableId`. The conversion is always safe, but the
/// result may be `undefined` or invalid in the specific BDD.
impl From<u32> for VariableId {
    fn from(value: u32) -> Self {
        VariableId(value)
    }
}

impl From<VariableId> for u32 {
    fn from(value: VariableId) -> Self {
        value.0
    }
}

impl From<VariableId> for u64 {
    fn from(value: VariableId) -> Self {
        value.0 as u64
    }
}

/// This conversion can lose information on 16-bit systems, but we enforce 64-bit compatibility
/// so should be fine as long as this crate compiles.
impl From<VariableId> for usize {
    fn from(value: VariableId) -> Self {
        value.0 as usize
    }
}