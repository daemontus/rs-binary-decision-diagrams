use cudd_sys::cudd::{
    Cudd_ReadLogicZero, Cudd_ReadOne, Cudd_ReadZero, Cudd_Ref, Cudd_bddIte, Cudd_bddIthVar,
};
use cudd_sys::DdNode;
use std::convert::TryFrom;
use std::os::raw::c_int;

/*
    64-bytes = cache line = 8 x u64 = 512 bits

    // Relatively well aligned (10-bit address is split between the two 4-byte values).
    // In terms of size, reasonable for most use cases that we can also do today.
    // The representation mostly works ok, because in resource intensive stuff, we usually
    // need all three unpacked values.
    8 nodes, 64-bit nodes, 10-bit variable, 27-bit address, 137M nodes, 1GB for largest BDD.
    // Alignment is a bit shit (10-bytes per node), and we are wasting 4 bytes per cache line.
    // But could be worth it compared to the next possible step in precision.
    // Also, we could fight the alignment with some fancier in-memory representation. Say that
    // addresses are stored at the beginning of the cache-line and variables at the end,
    // everything is aligned, and there is a 4-byte gap in the middle. But computing indices
    // will be a bit more complicated. Or separate in two vectors, but that loses the advantage
    // of having every node in one cache line.
    6 nodes, 80-bit nodes, 16-bit variable, 32-bit address, 4B nodes, 42GB for largest BDD.
    // Again, relatively ok alignment. Variable becomes part of the two addresses.
    // In terms of size, this is far more than you'd normally put in a PC today.
    4 nodes, 128-bit nodes, 32-bit variable, 48-bit address, several terabytes of RAM for the largest BDD.
    // Again, a 2x improvement over the 80-bit scheme. Alignment is still shit though.
    // It is nowhere near anything that we reasonably need right now.
    3 nodes, 160-bits, 32-bit variable, 64-bit address, all the memory in the world.

    Finally, we could just do a 64/64/64 representation, in which case we are trashing cache lines,
    but at least everything is aligned well and we save a few instruction at the expense of memory.
    Probably a reasonable tradeoff, with a 32/32/32 option for "packed" representation.

    The actual memory consumption of such a BDD would be only 1.5x larger than the 128-bit variant,
    but will effectively eliminate the need for a 3-node representation. Maybe we can create
    a backend API that will (reasonably) support swapping of internal representations.
 */

#[derive(Clone)]
pub struct Bdd {
    pub variable_count: u16,
    pub nodes: Vec<BddNode>,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct VariableId(pub u16);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NodeId(pub u64);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BddNode(pub VariableId, pub NodeId, pub NodeId);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PackedBddNode(pub u64, pub u64);

impl BddNode {
    pub fn variable(&self) -> VariableId {
        self.0
    }

    pub fn links(&self) -> (NodeId, NodeId) {
        (self.1, self.2)
    }

    pub fn low_link(&self) -> NodeId {
        self.1
    }

    pub fn high_link(&self) -> NodeId {
        self.2
    }

    pub fn pack(self) -> PackedBddNode {
        let packed_high = u64::from(self.2) | (u64::from(self.0 .0) << 48);
        PackedBddNode(u64::from(self.1), packed_high)
    }
}

impl From<NodeId> for u64 {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

impl From<u16> for VariableId {
    fn from(value: u16) -> Self {
        VariableId(value)
    }
}

impl VariableId {
    pub const UNDEFINED: VariableId = VariableId(u16::MAX);
}

impl NodeId {
    pub const ZERO: NodeId = NodeId(0);
    pub const ONE: NodeId = NodeId(1);
    pub const UNDEFINED: NodeId = NodeId(u64::MAX);

    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == u64::MAX
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.0 < 2
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_one(&self) -> bool {
        self.0 == 1
    }

    #[inline]
    pub unsafe fn as_index_unchecked(self) -> usize {
        self.0 as usize
    }
}

impl Bdd {
    /*#[inline]
    pub(crate) fn prefetch(&self, id: NodeId) {
        unsafe {
            // Prefetch operations ignore memory errors and are therefore "externally safe".
            if cfg!(target_arch = "x86_64") {
                let reference: *const BddNode = self.nodes.get_unchecked(id.0 as usize);
                std::arch::x86_64::_mm_prefetch::<3>(reference as *const i8);
            }
        }
    }*/

    pub fn variable_count(&self) -> u16 {
        self.variable_count
    }

    pub fn root_node(&self) -> NodeId {
        NodeId((self.nodes.len() - 1) as u64)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub unsafe fn get_node_unchecked(&self, id: NodeId) -> &BddNode {
        unsafe { self.nodes.get_unchecked(id.as_index_unchecked()) }
    }
}

impl Bdd {
    pub fn move_to_cudd(&self, manager: *mut cudd_sys::DdManager) -> *mut DdNode {
        let mut stack = Vec::with_capacity(2 * self.variable_count() as usize);
        stack.push(self.root_node());

        // This is actually not a BDD zero, but ADD/ZDD zero, so we use it as undef instead.
        let undef = unsafe { Cudd_ReadZero(manager) };
        let mut images: Vec<*mut DdNode> = vec![undef; self.node_count()];
        images[0] = unsafe { Cudd_ReadLogicZero(manager) };
        images[1] = unsafe { Cudd_ReadOne(manager) };

        while let Some(top) = stack.last() {
            let node = unsafe { self.get_node_unchecked(*top) };
            let dd_low = images[unsafe { node.low_link().as_index_unchecked() }];
            let dd_high = images[unsafe { node.high_link().as_index_unchecked() }];

            if dd_low != undef && dd_high != undef {
                let var_id: c_int = node.variable().0.into();
                let dd_var = unsafe { Cudd_bddIthVar(manager, var_id) };
                let dd_node = unsafe { Cudd_bddIte(manager, dd_var, dd_high, dd_low) };
                unsafe {
                    Cudd_Ref(dd_node);
                }
                let index = unsafe { top.as_index_unchecked() };
                images[index] = dd_node;
                stack.pop();
            } else {
                if dd_high == undef {
                    stack.push(node.high_link());
                }
                if dd_low == undef {
                    stack.push(node.low_link())
                }
            }
        }

        images[self.node_count() - 1]
    }

    pub fn sort_preorder_safe(&mut self) {
        if self.nodes.len() < 2 {
            return;
        }

        let mut new_id = vec![0usize; self.nodes.len()];
        new_id[0] = 0;
        new_id[1] = 1;

        let mut stack = Vec::new();
        stack.push(self.root_node());

        let mut new_index = self.nodes.len() - 1;
        while let Some(top) = stack.pop() {
            if top.is_terminal() {
                continue;
            }

            let current_index = unsafe { top.as_index_unchecked() };
            if new_id[current_index] == 0 {
                new_id[current_index] = new_index;
                new_index -= 1;

                let node = unsafe { self.get_node_unchecked(top) };
                stack.push(node.high_link());
                stack.push(node.low_link());
            }
        }

        assert_eq!(new_index, 1);

        let mut new_nodes = vec![BddNode(VariableId(0), NodeId(0), NodeId(0)); self.node_count()];

        for old_index in 0..self.node_count() {
            let node = unsafe { self.get_node_unchecked(NodeId(old_index as u64)) };
            let new_index = new_id[old_index];
            let new_low = new_id[unsafe { node.low_link().as_index_unchecked() }];
            let new_high = new_id[unsafe { node.high_link().as_index_unchecked() }];

            new_nodes[new_index] = BddNode(
                node.variable(),
                NodeId(new_low as u64),
                NodeId(new_high as u64),
            );
        }

        self.nodes = new_nodes;
    }

    pub fn sort_preorder(&mut self) {
        // TODO: There is a bug in this - probably about not transferring terminal nodes.
        if self.nodes.len() < 2 {
            return;
        }

        // Bdd sorted in pre-order is faster to iterate due to cache locality.
        let mut new_id = vec![0usize; self.nodes.len()];
        new_id[0] = 0;
        new_id[1] = 1;

        let mut stack_index_after_last: usize = 0;
        let mut stack = vec![NodeId::ZERO; 3 * usize::from(self.variable_count())];
        unsafe {
            *stack.get_unchecked_mut(stack_index_after_last) = self.root_node();
            stack_index_after_last += 1;
        }

        let mut new_index = self.nodes.len() - 1;
        while stack_index_after_last > 0 {
            let top = unsafe { *stack.get_unchecked(stack_index_after_last - 1) };
            stack_index_after_last -= 1;

            if top.is_one() || top.is_zero() {
                continue;
            }

            let index = unsafe { top.as_index_unchecked() };
            let new_id_cell = unsafe { new_id.get_unchecked_mut(index) };
            if *new_id_cell == 0 {
                *new_id_cell = new_index;
                new_index -= 1;
                let node = unsafe { self.get_node_unchecked(top) };
                unsafe {
                    *stack.get_unchecked_mut(stack_index_after_last) = node.high_link();
                    *stack.get_unchecked_mut(stack_index_after_last + 1) = node.low_link();
                    stack_index_after_last += 2;
                }
            }
        }

        let mut new_nodes = Vec::with_capacity(self.node_count());
        // Allocate nodes without initialization
        unsafe { new_nodes.set_len(self.node_count()) };
        for old_index in 2..new_id.len() {
            let node = unsafe { self.nodes.get_unchecked(old_index) };

            let new_index = unsafe { *new_id.get_unchecked(old_index) };
            let new_low = unsafe { *new_id.get_unchecked(node.low_link().as_index_unchecked()) };
            let new_high = unsafe { *new_id.get_unchecked(node.high_link().as_index_unchecked()) };
            unsafe {
                let cell = new_nodes.get_unchecked_mut(new_index);
                *cell = BddNode(node.0, NodeId(new_low as u64), NodeId(new_high as u64));
                //*cell = BddNode(node.0, NodeId(new_high as u64), NodeId(new_low as u64));
            }
        }

        self.nodes = new_nodes;
    }
}

impl TryFrom<&str> for Bdd {
    type Error = String;

    fn try_from(data: &str) -> Result<Self, Self::Error> {
        //let mut node_variables = Vec::new();
        //let mut node_pointers = Vec::new();
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
            let variable = if let Ok(x) = variable.unwrap().parse::<u16>() {
                x
            } else {
                return Err(format!("Invalid variable numeral `{}`.", variable.unwrap()));
            };
            let low_pointer = if let Ok(x) = left_pointer.unwrap().parse::<u64>() {
                x
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    left_pointer.unwrap()
                ));
            };
            let high_pointer = if let Ok(x) = right_pointer.unwrap().parse::<u64>() {
                x
            } else {
                return Err(format!(
                    "Invalid pointer numeral `{}`.",
                    right_pointer.unwrap()
                ));
            };
            nodes.push(BddNode(
                VariableId(variable),
                NodeId(low_pointer),
                NodeId(high_pointer),
            ));
            //nodes.push(BddNode(VariableId(variable), NodeId(high_pointer), NodeId(low_pointer)));
        }
        Ok(Bdd {
            variable_count: nodes[0].0 .0,
            nodes,
        })
    }
}
