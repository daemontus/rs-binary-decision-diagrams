use crate::IntoIndex;

/// A unique 32-bit numeric identifier of a Boolean BDD variable.
///
/// The max. value is reserved for the `UNDEFINED` value, so the actual maximal number of
/// variables is `2^32 - 1`. A variable which is not undefined is valid within any BDD.
/// However the BDD may not impose any restrictions on said variable. To find out if a variable
/// is truly used in the BDD, one has to iterate through its nodes.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Variable(u32);

impl Variable {
    pub const UNDEFINED: Variable = Variable(u32::MAX);

    #[inline]
    pub fn is_undefined(&self) -> bool {
        *self == Self::UNDEFINED
    }

}

impl From<u32> for Variable {
    fn from(value: u32) -> Self {
        Variable(value)
    }
}

impl From<Variable> for u32 {
    fn from(value: Variable) -> Self {
        value.0
    }
}

impl IntoIndex for Variable {
    fn into_index(self) -> usize {
        self.0.into_index()
    }
}