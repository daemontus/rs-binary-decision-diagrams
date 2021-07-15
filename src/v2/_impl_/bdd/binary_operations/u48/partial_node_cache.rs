use crate::v2::{Bdd, BddNode, NodeId, VariableId};
use std::convert::TryFrom;
use std::num::NonZeroU64;
use std::ops::{BitXor, Rem};

/// **(internal)** A partial hash map which handles uniqueness queries for the nodes of a `Bdd`.
/// It owns the result `Bdd` into which all the nodes are stored (without leaking).
///
/// It is a hash map which overwrites on collision, just as `TaskCache`, but it keeps the keys
/// in the result `Bdd`, avoiding double allocation. We also assume that `NodeId::ZERO` is never
/// saved into the cache (since it has a static position) and thus we can use it as an undefined
/// value to speed up initial allocation.
pub struct NodeCache {
    /*
       A little horror story for you: For some reason, if you try to keep `nodes`
       outside of this struct (have it as an object in the main procedure), the
       compiler has a little fit and the whole thing becomes slower. Not sure why,
       maybe it has something to do with caches, or maybe the reduced number of
       arguments to `ensure` lessens the register pressure. Who knows. Just beware
       and measure if you want to optimize something.
    */
    capacity: NonZeroU64,
    nodes: Bdd,
    // Every value is either `NodeId::ZERO` or a valid pointer into `nodes`.
    values: Vec<NodeId>,
}

impl NodeCache {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    /// **(internal)** Create a new node cache backed by a `Bdd`. The capacity of the `Bdd` will
    /// extend if needed, but the capacity of the hash table is fixed.
    pub fn new(capacity: usize) -> NodeCache {
        debug_assert!(capacity > 0);
        NodeCache {
            nodes: Bdd::true_with_capacity(capacity),
            values: vec![NodeId::ZERO; capacity],
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
        }
    }

    /// **(internal)** Ensure that the backing `Bdd` contains the given node. If not,
    /// the node is created. Returns a valid id of the existing or created node.
    #[inline]
    pub fn ensure(&mut self, node: BddNode) -> NodeId {
        let index = self.hash(node);
        let entry = unsafe { self.values.get_unchecked_mut(index) };
        let candidate_id = *entry;
        if !candidate_id.is_zero() && self.nodes.get_node(candidate_id) == node {
            candidate_id
        } else {
            let new_id = self.nodes.push_node(node);
            *entry = new_id;
            new_id
        }
    }

    /// Finalize this cache and return the final `Bdd` object.
    #[inline]
    pub fn export(self) -> Bdd {
        self.nodes
    }

    /// **(internal)** A hash function inspired by Knuth and FxHash.
    ///
    /// Always returns a valid index into `self.values`, hence no need to
    /// check bounds when using it.
    #[inline]
    fn hash(&self, node: BddNode) -> usize {
        let left = node.0.wrapping_mul(Self::SEED);
        let right = node.1.wrapping_mul(Self::SEED);
        left.bitxor(right).rem(self.capacity) as usize
    }
}
