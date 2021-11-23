use super::Variable;
use super::NodeIndex;

/// A collection of values which together describe a BDD decision node.
///
/// A BDD node is described by a BDD variable, and indices of its two child nodes (low and high
/// decision branches). You can assume that these values are never undefined, except for one
/// special case: the terminal nodes.
///
/// For terminal nodes, we assume that the low/high nodes are simply self-loops, while the variable
/// is set to `Variable::UNDEFINED`. By default, we assume two terminal nodes (`0` and `1`).
/// However, this also allows us to introduce more terminal nodes in the future, with the
/// assumption that a terminal node satisfies this property (variable is undefined, children are
/// self-loops) and is it sorted before any non-terminal nodes in the BDD.
///
/// Also note that we assume a BDD is sorted in such a way that smaller variables are closer to the
/// root. This means a `Variable::UNDEFINED` in the terminal node is always the correct termination
/// of any growing sequence of variables that one may encounter on any path in the BDD.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Node(Variable, NodeIndex, NodeIndex);

impl Node {
    pub const ZERO: Node = Node(Variable::UNDEFINED, NodeIndex::ZERO, NodeIndex::ZERO);
    pub const ONE: Node = Node(Variable::UNDEFINED, NodeIndex::ONE, NodeIndex::ONE);

    #[inline]
    pub fn pack(variable: Variable, low: NodeIndex, high: NodeIndex) -> Node {
        Node(variable, low, high)
    }

    #[inline]
    pub fn unpack(&self) -> (Variable, NodeIndex, NodeIndex) {
        (self.0, self.1, self.2)
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.0.is_undefined()
    }

    #[inline]
    pub fn get_variable(&self) -> Variable {
        self.0
    }

    #[inline]
    pub fn get_low_link(&self) -> NodeIndex {
        self.1
    }

    #[inline]
    pub fn get_high_link(&self) -> NodeIndex {
        self.2
    }
}