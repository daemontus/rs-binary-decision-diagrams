use super::{Node, Variable, NodeIndex};
use crate::{FromIndex, IntoIndex};
use std::cmp::max;
use std::collections::VecDeque;
use std::iter::Map;
use std::ops::Range;
use std::convert::TryFrom;

/// A `Bdd` describes a directed acyclic graph corresponding to a Boolean function.
///
/// There are two "orderings" that apply to the BDD object: A topological ordering of nodes
/// facilitated through the low and high edges, and an actual "physical" order in which the nodes
/// are stored in the `nodes` vector.
///
/// Topologically, we assume that the root contains the smallest variable, and the decision
/// variables grow towards the terminal nodes. Additionally, when we talk about the pre-order
/// or post-order of the graph, we assume that the low edge is explored before the high edge.
///
/// In the `nodes` vector, the nodes are sorted such that the root is the very last node, and
/// the terminal nodes are the very first nodes. You should not assume any other properties
/// for the order of nodes in the BDD. However, for performance reasons, we often impose a "soft"
/// assumption that the BDD nodes are sorted based on the topological pre-order of the graph.
/// In such a case, the algorithm must still be correct for any valid ordering of nodes, but
/// it may be slower in cases where the ordering is different from what is expected.
///
/// Also note that this implies the `nodes` vector is never empty, because it must always contain
/// at least the `0` terminal node. However, note that we also do not require that every node
/// in the vector must be reachable from the root. This is useful for operations that may make
/// some nodes unreachable, but where it is not strictly necessary to minimize the graph after
/// the operation is completed (usually for performance reasons).
///
/// Aside from the nodes, the BDD also keeps track of its height, which is the number of nodes
/// on the longest path in the BDD (so a BDD with only terminal nodes has height `0`, and a BDD
/// with a path which actually uses every valid variable has height `u32::MAX`). This value is
/// again useful in some algorithms as it represents an upper bound on the stack size which is
/// needed to explore the BDD. However, the value is not required to be precise. For its intended
/// purpose, any number larger or equal to the actual height is correct. When convenient, the
/// implementations can thus choose to only provide a reasonable upper bound on the graph height.
/// Please do avoid simply setting the height to `u32::MAX` though; this can significantly increase
/// the memory consumption of the BDD manipulation algorithms.
///
/// To simplify height approximation, it is recommended one allocates new variables in descending
/// order (starting with `2^32 - 1`, ending with `0`). When such a scheme is implemented,
/// `u32::MAX - root_node.variable` is a reasonable approximation of the BDD height. But a better
/// approximation may still be necessary if your application frequently uses only a small subset
/// of allocated variables.
///
#[derive(Clone)]
pub struct Bdd {
    height: u32,
    nodes: Vec<Node>
}

type NodeIndexIterator = Map<Range<usize>, fn(usize) -> NodeIndex>;

/// Basic operations for examining the contents of a BDD.
impl Bdd {

    /// An unsafe constructor for creating BDDs directly from node vectors.
    pub unsafe fn from_raw_parts(height: u32, nodes: Vec<Node>) -> Bdd {
        Bdd { height, nodes }
    }

    /// Create a BDD from a vector of nodes. The height will be computed using a BFS search.
    ///
    /// *Panics:* The nodes must form a valid BDD in terms of `Bdd::is_valid_bdd`.
    pub fn from_nodes(nodes: Vec<Node>) -> Bdd {
        assert!(Bdd::check_consistency_errors(&nodes).is_none());
        let mut bdd = unsafe { Bdd::from_raw_parts(u32::MAX, nodes) };
        bdd.recompute_height();
        bdd
    }

    /// Create a BDD representing the constant `0`.
    pub fn new_zero() -> Bdd {
        Bdd {
            height: 0,
            nodes: vec![Node::ZERO]
        }
    }

    /// Create a BDD representing the constant `1`.
    pub fn new_one() -> Bdd {
        Bdd {
            height: 0,
            nodes: vec![Node::ZERO, Node::ONE]
        }
    }

    /// Upper bound on the height of the BDD graph.
    #[inline]
    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// The number of nodes in the BDD graph.
    #[inline]
    pub fn get_size(&self) -> u64 {
        u64::from_index(self.nodes.len())
    }

    /// The index of the graph root node.
    ///
    /// *This value cannot be undefined since even constant BDDs have at least one terminal node.*
    #[inline]
    pub fn get_root_index(&self) -> NodeIndex {
        NodeIndex::from_index(self.nodes.len() - 1)
    }

    /// A reference to the graph root node.
    ///
    /// *This value cannot be undefined since even constant BDDs have at least one terminal node.*
    #[inline]
    pub fn get_root_node(&self) -> &Node {
        let root = self.get_root_index();
        // This is safe because a BDD is never empty.
        unsafe { self.get_node_unchecked(root) }
    }

    /// Get a reference to a `Node` using the given `index`.
    #[inline]
    pub fn get_node(&self, index: NodeIndex) -> &Node {
        &self.nodes[index.into_index()]
    }

    /// Get a reference to a `Node` using the given `index` without checking bounds.
    #[inline]
    pub unsafe fn get_node_unchecked(&self, index: NodeIndex) -> &Node {
        unsafe { self.nodes.get_unchecked(index.into_index()) }
    }

    /// Create an iterator over all node indices of this BDD.
    #[inline]
    pub fn iter_indices(&self) -> NodeIndexIterator {
        (0..self.nodes.len()).map(|it| NodeIndex::from_index(it))
    }

    /// True if the BDD represents a constant (terminal) value.
    #[inline]
    pub fn is_constant(&self) -> bool {
        self.get_root_node().is_terminal()
    }

    /// True if the BDD represents a constant zero value.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.nodes.len() == 1
    }

    /// True if the BDD represents a constant one value.
    #[inline]
    pub fn is_one(&self) -> bool {
        self.nodes.len() == 2
    }

}

/// Some useful validation and normalization methods.
impl Bdd {

    /// Dynamically verify that the given slice of nodes can be safely interpreted as a BDD.
    ///
    /// In case of error, returns an error string.
    pub fn check_consistency_errors(nodes: &[Node]) -> Option<String> {
        let root = NodeIndex::from_index(nodes.len() - 1);
        for node in nodes {
            // Every node links to a valid node in the BDD.
            if node.get_low_link() > root {
                return Some(format!("Low link {:?} is out of bounds ({:?} is root).", node.get_low_link(), root));
            }
            if node.get_high_link() > root {
                return Some(format!("High link {:?} is out of bounds ({:?} is root).", node.get_high_link(), root));
            }
            let low = &nodes[node.get_low_link().into_index()];
            let high = &nodes[node.get_high_link().into_index()];
            // And the links preserve variable ordering.
            if !node.is_terminal() {
                if low.get_variable() <= node.get_variable() {
                    return Some(format!("Low link in {:?} violates variable order ({:?} <= {:?}).", node, low.get_variable(), node.get_variable()));
                }
                if high.get_variable() <= node.get_variable() {
                    return Some(format!("High link in {:?} violates variable order ({:?} <= {:?}).", node, high.get_variable(), node.get_variable()));
                }
            }
            // Low and high should be self loops if and only if the node is a terminal:
            if (low == node) != node.is_terminal() {
                return Some(format!("Low self-loop violation in {:?}.", node));
            }
            if (high == node) != node.is_terminal() {
                return Some(format!("High self-loop violation in {:?}.", node));
            }
        }

        // Count terminal nodes:
        let terminals = nodes.iter().take_while(|it| it.is_terminal()).count();

        // Check if all terminals are the first nodes in the slice:
        let all_terminals = nodes.iter().filter(|it| it.is_terminal()).count();
        if terminals != all_terminals {
            return Some(format!("Terminal node order violation. All terminals: {}. Correct terminals: {}.", all_terminals, terminals));
        }

        // At this point, we know that every reference in the slice is valid, the terminal nodes
        // are at the beginning of the slice, and self-loops are exactly on terminal nodes.
        return None;
    }


    /// Update the height value of this BDD with a true height obtained using a BFS search.
    pub fn recompute_height(&mut self) {
        // Constant BDD has height 0.
        if self.is_constant() {
            self.height = 0;
            return;
        }

        // Run a BFS through all nodes, determining the exact maximal height for each of them.
        let mut height: Vec<u32> = vec![0; self.nodes.len()];
        let mut visited: Vec<bool> = vec![false; self.nodes.len()];
        let mut queue: VecDeque<NodeIndex> = VecDeque::with_capacity(128);

        queue.push_back(self.get_root_index());
        height[self.get_root_index().into_index()] = 1;

        while let Some(i_node) = queue.pop_front() {
            let node = self.get_node(i_node);
            let node_height = height[i_node.into_index()];

            let low_link = node.get_low_link().into_index();
            let high_link = node.get_high_link().into_index();
            height[low_link] = max(height[low_link], node_height + 1);
            height[high_link] = max(height[high_link], node_height + 1);

            // If visited is true, the node is already somewhere in the queue and will be
            // popped eventually.
            if !visited[low_link] {
                visited[low_link] = true;
                queue.push_back(node.get_low_link());
            }
            if !visited[high_link] {
                visited[high_link] = true;
                queue.push_back(node.get_high_link());
            }
        }

        // The final height is the maximum among the terminal nodes:
        self.height = 0;
        for node in self.iter_indices() {
            if !self.nodes[node.into_index()].is_terminal() {
                return;
            }

            self.height = max(self.height, height[node.into_index()]);
        }
    }

    /// Create a copy of the BDD by reordering the nodes based on the provided shuffle vector.
    ///
    /// The assumption is that the vector contains every node index of the BDD exactly once,
    /// and that the final shuffle preserves the BDD invariants (mainly that terminal nodes
    /// will not be shuffled in between the decision nodes).
    pub unsafe fn shuffle_unchecked(&self, shuffle: &[NodeIndex]) -> Bdd {
        if self.is_constant() {
            // Constant BDDs cannot be shuffled.
            return self.clone();
        }
        // Just a tiny sanity check.
        debug_assert_eq!(self.nodes.len(), shuffle.len());

        // A trick which avoids unnecessary memory initialization.
        let mut new_nodes = Vec::with_capacity(self.nodes.len());
        unsafe { new_nodes.set_len(self.nodes.len()); }

        // Remap nodes into the new vector.
        for (old_index, new_index) in shuffle.iter().enumerate() {
            let old_node = self.get_node(NodeIndex::from_index(old_index));
            let new_low = shuffle[old_node.get_low_link().into_index()];
            let new_high = shuffle[old_node.get_high_link().into_index()];
            new_nodes[new_index.into_index()] = Node::pack(old_node.get_variable(), new_low, new_high);
        }

        Bdd {
            height: self.height,
            nodes: new_nodes,
        }
    }

    /// Create a copy of this `Bdd` that is sorted based on the DFS pre-order.
    pub fn sort_preorder(&self) -> Bdd {
        if self.is_constant() { // Skip for trivial BDDs.
            return self.clone();
        }

        let mut shuffle_map = vec![NodeIndex::UNDEFINED; self.nodes.len()];

        // Initialize the shuffle map to preserve all terminal nodes in their places.
        let terminals_count = self.nodes.iter().take_while(|it| it.is_terminal()).count();
        for i in 0..terminals_count {
            shuffle_map[i] = NodeIndex::from_index(i);
        }

        let mut search_stack: Vec<NodeIndex> = Vec::with_capacity(self.height.into_index());
        search_stack.push(self.get_root_index());

        // Populate `id_map` based on DFS preorder.
        let mut next_free_index = self.nodes.len() - 1;
        while let Some(task) = search_stack.pop() {
            let task_id = shuffle_map[task.into_index()];
            if task_id.is_undefined() {
                shuffle_map[task.into_index()] = NodeIndex::from_index(next_free_index);
                next_free_index -= 1;

                let node = self.get_node(task);
                search_stack.push(node.get_high_link());
                search_stack.push(node.get_low_link());
            }
        }

        // Every node should have been assigned a new index except for the terminals,
        // which we fixed in the beginning.
        debug_assert_eq!(terminals_count - 1, next_free_index);
        unsafe { self.shuffle_unchecked(&shuffle_map) }
    }

    /// Create a copy of this `Bdd` that is sorted based on the DFS post-order.
    pub fn sort_postorder(&self) -> Bdd {
        if self.is_constant() { // Skip for trivial BDDs.
            return self.clone();
        }

        let mut shuffle_map = vec![NodeIndex::UNDEFINED; self.nodes.len()];

        // Initialize the shuffle map to preserve all terminal nodes in their places.
        let terminals_count = self.nodes.iter().take_while(|it| it.is_terminal()).count();
        for i in 0..terminals_count {
            shuffle_map[i] = NodeIndex::from_index(i);
        }

        let mut search_stack: Vec<(NodeIndex, bool)> = Vec::with_capacity(self.height.into_index());
        search_stack.push((self.get_root_index(), false));

        // First non-terminal index that can be assigned:
        let mut next_free_index = terminals_count;
        while let Some((task, expended)) = search_stack.pop() {
            let task_id = shuffle_map[task.into_index()];
            if expended {
                // All children are exported and the task can get its ID now:
                shuffle_map[task.into_index()] = NodeIndex::from_index(next_free_index);
                next_free_index += 1;
            } else if task_id.is_undefined() {
                // Task is not expanded and the result is so far unknown.
                let node = self.get_node(task);
                search_stack.push((task, true));
                search_stack.push((node.get_high_link(), false));
                search_stack.push((node.get_low_link(), false));
            }
        }

        assert_eq!(next_free_index, self.nodes.len());
        unsafe { self.shuffle_unchecked(&shuffle_map) }
    }

}

/// Deserialization of a simple string format for sharing BDDs.
impl TryFrom<&str> for Bdd {
    type Error = String;

    fn try_from(data: &str) -> Result<Self, Self::Error> {
        let mut nodes = Vec::new();
        for node_string in data.split('|').filter(|s| !s.is_empty()) {
            let mut node_items = node_string.split(',');
            let variable = node_items.next();
            let left_pointer = node_items.next();
            let right_pointer = node_items.next();
            if node_items.next().is_some()
                || variable.is_none()
                || left_pointer.is_none()
                || right_pointer.is_none()
            {
                return Err(format!("Unexpected node representation `{}`.", node_string));
            }
            let variable = if let Ok(x) = variable.unwrap().parse::<u32>() {
                Variable::from(x)
            } else {
                return Err(format!("Invalid variable numeral `{}`.", variable.unwrap()));
            };
            let low_pointer = if let Ok(x) = left_pointer.unwrap().parse::<u64>() {
                NodeIndex::from(x)
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    left_pointer.unwrap()
                ));
            };
            let high_pointer = if let Ok(x) = right_pointer.unwrap().parse::<u64>() {
                NodeIndex::from(x)
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    right_pointer.unwrap()
                ));
            };
            nodes.push(Node::pack(variable, low_pointer, high_pointer));
        }
        // Replace terminals because the files currently use the old format:
        if let Some(zero) = nodes.get_mut(0) {
            *zero = Node::ZERO;
        }
        if let Some(one) = nodes.get_mut(1) {
            *one = Node::ONE;
        }
        if let Some(error) = Bdd::check_consistency_errors(&nodes) {
            Err(error)
        } else {
            let mut bdd = unsafe { Bdd::from_raw_parts(u32::MAX, nodes) };
            bdd.recompute_height();
            Ok(bdd)
        }
    }
}


#[cfg(test)]
mod test {
    use super::Bdd;
    use std::convert::TryFrom;

    #[test]
    pub fn basic_sorting_test() {
        let bdd = std::fs::read_to_string("bench_inputs/itgr/large-large-large.109.and_not.left.bdd").unwrap();
        let bdd = Bdd::try_from(bdd.as_str()).unwrap();

        // Note that initially, the BDD is in post-order, but sorted from high to low. Our
        // postorder is from low to high, so neither sort actually corresponds to the original
        // file format.

        let preorder = bdd.sort_preorder();
        let postorder = preorder.sort_postorder();

        assert_ne!(bdd.nodes, preorder.nodes);
        assert_ne!(bdd.nodes, postorder.nodes);
        assert_eq!(preorder.nodes, postorder.sort_preorder().nodes);
        assert_eq!(postorder.nodes, preorder.sort_postorder().nodes);
    }

}