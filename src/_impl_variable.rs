use crate::Variable;

impl From<u16> for Variable {
    fn from(value: u16) -> Self {
        Variable(value)
    }
}