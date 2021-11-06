/// A packed representation of a BDD node.
pub mod packed_bdd_node;
/// A unique integer identifier of a BDD variable, up to 32 bits.
pub mod variable_id;
/// A unique integer identifier of a BDD node, up to 64 bits.
pub mod node_id;
/// A linear in-memory representation of the BDD graph.
pub mod bdd;

/// A module for the internal data structures of the out-of-order algorithm.
pub mod ooo;