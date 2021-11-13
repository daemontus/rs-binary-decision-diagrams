use crate::v2::NodeId;
use std::cmp::max;
use std::convert::TryFrom;
use std::num::NonZeroU64;
use std::ops::{BitXor, Rem};

/// **(internal)** A partial hash map which saves the results of already processed tasks.
///
/// It is essentially a hash map which overwrites on collision to avoid costly branches.
/// It relies on the fact that task (0,0) should be always resolved using a lookup table
/// and will therefore never appear as a key in the cache. This way, we can start by
/// zeroing all the memory, which appears to be slightly faster on x86 for some reason.
///
/// More importantly, the cache uses a special hashing strategy. It assumes the BDDs
/// which generate the tasks are explored more-or-less predictably in a DFS-preorder.
/// (This is never really true, but it is close enough to provide a useful assumption)
/// As a result, the sequence of queries on the cache is biased towards being decreasing
/// (since BDD is indexed from high to low). We can exploit this bias by constructing
/// the cache as a sequence of small, overlaying hash tables, each fitting into the L3
/// cache. Each hash table begins at the index given by the pointer into the *larger*
/// BDD (as this is the capacity of the whole cache). Then, both pointers determine the
/// index in the hash table that begins at that position.
///
/// As a consequence, with decreasing the larger pointer, the small hash table "rolls"
/// through the available space in a predictable fashion. If the whole table fits into
/// L3, the effect of this is mostly negligible. However, as soon as the table becomes
/// too big and has to be moved into RAM (say, 100k nodes), the overall impact can be
/// drastic (30+% speedup). All of this works best when the BDD is sorted in DFS-preorder,
/// but there is a measurable improvement also in other orderings (such as postorder).
/// in particular, it appears that the benefits of sorted iteration outweigh the price
/// of sorting, especially when the BDD is used in more than one operation.
///
/// Final note: In our applications which use saturation and overall do a lot of
/// reachability, we expect a significant portion of the BDDs to be reused and quite
/// a lot of them will end up empty. Empty result is very good because it does not
/// have to be sorted.
///
/// TODO:
/// This strategy relies on the fact that the *smaller* BDD is small enough such that
/// the collisions in the rolling hash table will not be too drastic. What we should
/// in fact be doing is a "two-level-rolling". I.e. *larger* pointer determines a
/// position of a "super block" (the size of which is the smaller BDD) and *smaller*
/// pointer determines the position of the actual hash table in that "super block".
/// (Assuming the super block is bigger than the table. If not, it's just one table)
/// This way, the window is moving predictably with respect to both pointers and the
/// size of the block can be an (essentially) arbitrary constant.
pub(super) struct TaskCache {
    capacity: NonZeroU64,
    keys: Vec<(NodeId, NodeId)>,
    values: Vec<NodeId>,
}

impl TaskCache {
    // TODO:
    // This number is essentially determined as "large enough to avoid too many collisions
    // but small enough to fit into L3 cache". With the current implementation, it stands
    // at ~400kB. However, it would be nice if we can determine it dynamically. Especially
    // in cases where the two BDDs are very big and we expect a lot of collisions even in
    // this space.
    // See also: https://docs.rs/cache-size/0.5.1/cache_size/
    const HASH_BLOCK: u64 = 1 << 14;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    /// **(internal)** Create a new `TaskCache` with the given fixed (non-zero!) capacity.
    ///
    /// Note that we expect the *left* size to be larger than the *right* size, due to
    /// the way our hashing algorithm works.
    pub fn new(left_size: usize, right_size: usize) -> TaskCache {
        debug_assert!(left_size >= right_size);
        let capacity = max(left_size, right_size);
        TaskCache {
            capacity: NonZeroU64::new(u64::try_from(capacity).unwrap()).unwrap(),
            keys: vec![(NodeId::ZERO, NodeId::ZERO); capacity],
            values: vec![NodeId::ZERO; capacity],
        }
    }

    /// **(internal)** Read an entry from the cache. If the entry is not present,
    /// returns `NodeId::UNDEFINED`.
    #[inline]
    pub fn read(&self, left: NodeId, right: NodeId) -> NodeId {
        let index = self.hashed_index(left, right);
        unsafe {
            if *self.keys.get_unchecked(index) == (left, right) {
                *self.values.get_unchecked(index)
            } else {
                NodeId::UNDEFINED
            }
        }
    }

    /// **(internal)** Write a new entry into the cache.
    #[inline]
    pub fn write(&mut self, left: NodeId, right: NodeId, result: NodeId) {
        let index = self.hashed_index(left, right);
        unsafe {
            let key = self.keys.get_unchecked_mut(index);
            let value = self.values.get_unchecked_mut(index);
            *key = (left, right);
            *value = result;
        }
    }

    /// **(internal)** Prefetch the given entry if possible.
    #[inline]
    pub fn prefetch(&self, left: NodeId, right: NodeId) {

    }

    /// **(internal)** A hash function partially inspired by Knuth and FxHash.
    ///
    /// Always returns a valid index into `self.keys` and `self.values`, hence no need to
    /// check bounds when using it.
    #[inline]
    fn hashed_index(&self, left: NodeId, right: NodeId) -> usize {
        let left_hash = u64::from(left).wrapping_mul(Self::SEED);
        let right_hash = u64::from(right).wrapping_mul(Self::SEED);
        let block_index = left_hash.bitxor(right_hash).rem(Self::HASH_BLOCK);
        (left.0 + block_index).rem(self.capacity) as usize
    }
}
