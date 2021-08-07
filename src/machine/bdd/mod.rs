use super::BddNode;
use crate::machine::NodeId;
use std::cmp::max;
use std::convert::TryFrom;
use std::ops::Index;

/// A directed acyclic graph representing a Boolean function.
///
/// The first two nodes must be `ZERO` and `ONE`. The root node must be last.
/// Otherwise, the memory-ordering of nodes is arbitrary (there may even be unused/dead nodes).
/// However, we try to keep the nodes sorted according to the DFS preorder relation whenever
/// possible because it makes BDD iteration faster.
///
/// In terms of topological ordering, we assume variables are ordered with (numerically) smallest
/// variable ids in the root. This aligns with the fact that decision variable in leaves is
/// undefined, which in greater than any valid variable.
#[derive(Clone, Debug)]
pub struct Bdd {
    variable_count: u16,
    nodes: Vec<BddNode>,
}

impl Bdd {
    /// Create a new `Bdd` with no variables that represents a `false` formula.
    pub fn new_false() -> Bdd {
        Bdd {
            variable_count: 0,
            nodes: vec![BddNode::ZERO],
        }
    }

    /// Create a new `Bdd` with no variables that represents a `true` formula.
    pub fn new_true() -> Bdd {
        Bdd {
            variable_count: 0,
            nodes: vec![BddNode::ZERO, BddNode::ONE],
        }
    }

    /// Get the number of nodes in this `Bdd`.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of variables in this `Bdd`.
    pub fn variable_count(&self) -> u16 {
        self.variable_count
    }

    /// Get the id of the root node in this `Bdd`.
    pub fn root_id(&self) -> NodeId {
        // Assuming all goes well, a `Bdd` cannot outgrow a 48 bits address space,
        // so unchecked conversion to u64 is ok here.
        unsafe { NodeId::from_u64((self.nodes.len() as u64) - 1) }
    }

    /// A safe function for reading nodes stored in this `Bdd`.
    ///
    /// If you can afford a panic, you can also use direct indexing.
    pub fn get_node(&self, id: NodeId) -> Option<&BddNode> {
        let index = usize::try_from(id.into_u64()).unwrap();
        self.nodes.get(index)
    }

    /// Ensure that the `Bdd` admits at least `count` variables.
    pub fn ensure_variable_count(&mut self, count: u16) {
        self.variable_count = max(self.variable_count, count)
    }

    /// An unchecked version of `Bdd::get_node` intended for performance critical code.
    pub unsafe fn get_node_unchecked(&self, id: NodeId) -> &BddNode {
        unsafe { self.nodes.get_unchecked(id.into_usize()) }
    }

    /// Append the given `node` to the `Bdd` as a new root node, without consistency checks.
    ///
    /// If you use this function, you must ensure that:
    ///  - The `Bdd` has enough variables.
    ///  - The links on the inserted node are valid in this `Bdd`.
    ///  - The node does not break variable ordering (i.e. it only links to *greater* variables).
    ///
    /// The last condition cannot be easily checked at runtime, thus this function cannot be safe.
    pub unsafe fn push_node(&mut self, node: BddNode) -> NodeId {
        debug_assert!(self.variable_count > u16::from(node.variable()));
        debug_assert!(node.low_link().into_u64() < self.nodes.len() as u64);
        debug_assert!(node.high_link().into_u64() < self.nodes.len() as u64);
        self.nodes.push(node);
        self.root_id()
    }

    /// Checks for "syntactic" equality between two `Bdd` objects.
    ///
    /// This is more strict than logical equivalence because two `Bdd` objects can represent the
    /// same function using a different in-memory ordering of nodes.
    pub fn eq_bytes(&self, other: &Bdd) -> bool {
        self.variable_count == other.variable_count && self.nodes == other.nodes
    }
}

impl Index<NodeId> for Bdd {
    type Output = BddNode;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.get_node(index).unwrap()
    }
}

#[cfg(test)]
mod tests {

    use super::super::{BddNode, NodeId, VariableId};
    use super::Bdd;

    #[test]
    fn basic_bdd_operations() {
        let mut bdd = Bdd::new_true();
        assert!(!Bdd::new_false().eq_bytes(&bdd));
        assert_eq!(2, bdd.node_count());
        assert_eq!(0, bdd.variable_count());
        bdd.ensure_variable_count(10);
        assert_eq!(10, bdd.variable_count());
        let node = BddNode::try_pack(VariableId::from(7), NodeId::ONE, NodeId::ZERO).unwrap();
        let inserted = unsafe { bdd.push_node(node) };
        assert_eq!(3, bdd.node_count());
        assert_eq!(inserted, bdd.root_id());
        assert_eq!(bdd[inserted], node);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn bdd_add_invalid_variable() {
        let mut bdd = Bdd::new_true();
        let node = BddNode::try_pack(VariableId::from(3), NodeId::ONE, NodeId::ZERO).unwrap();
        unsafe {
            bdd.push_node(node);
        }
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn bdd_add_invalid_low_link() {
        let mut bdd = Bdd::new_true();
        bdd.ensure_variable_count(10);
        let node =
            BddNode::try_pack(VariableId::from(3), NodeId::from_u48(32), NodeId::ZERO).unwrap();
        unsafe {
            bdd.push_node(node);
        }
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn bdd_add_invalid_high_link() {
        let mut bdd = Bdd::new_true();
        bdd.ensure_variable_count(10);
        let node =
            BddNode::try_pack(VariableId::from(3), NodeId::ONE, NodeId::from_u48(32)).unwrap();
        unsafe {
            bdd.push_node(node);
        }
    }
}
