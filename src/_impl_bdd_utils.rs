use crate::{Bdd, Variable, Pointer, PointerPair};
use std::convert::TryFrom;
use std::cmp::max;

impl Bdd {

    pub fn new_false() -> Bdd {
        /*Bdd {
            node_variables: vec![Variable::from(0)],
            node_pointers: vec![Pointer::zero() | Pointer::zero()]
        }*/
        Bdd {
            nodes: vec![(0, 0, Variable::from(0), Pointer::zero() | Pointer::zero())]
        }
    }

    /*pub fn new_true() -> Bdd {
        Bdd {
            node_variables: vec![
                Variable::from(0),
                Variable::from(0)
            ],
            node_pointers: vec![
                Pointer::zero() | Pointer::zero(),
                Pointer::one() | Pointer::one(),
            ]
        }
    }*/

    pub fn new_true_with_variables(variable_count: u16) -> Bdd {
        /*Bdd {
            node_variables: vec![
                Variable::from(variable_count), Variable::from(variable_count)
            ],
            node_pointers: vec![
                Pointer::zero() | Pointer::zero(),
                Pointer::one() | Pointer::one(),
            ]
        }*/
        Bdd {
            nodes: vec![
                (0,0,Variable::from(variable_count), Pointer::zero() | Pointer::zero()),
                (0,0,Variable::from(variable_count), Pointer::one() | Pointer::one()),
            ]
        }
    }

    #[inline]
    pub(crate) fn create_node(&mut self, variable: Variable, low: Pointer, high: Pointer) -> Pointer {
        //self.node_variables.push(variable);
        //self.node_pointers.push(low | high);
        //Pointer((self.node_pointers.len() - 1) as u32)
        self.nodes.push((0,0,variable, low | high));
        Pointer((self.nodes.len() - 1) as u32)
    }

    pub(crate) fn push_node(&mut self, variable: Variable, pointers: PointerPair) {
        //self.node_variables.push(variable);
        //self.node_pointers.push(pointers);
        self.nodes.push((0,0,variable, pointers));
    }

    pub fn variable_count(&self) -> u16 {
        //unsafe { self.node_variables.get_unchecked(0).0 }
        unsafe { self.nodes.get_unchecked(0).2.0 }
    }

    #[inline]
    pub(crate) fn var_of(&self, pointer: Pointer) -> Variable {
        unsafe { self.nodes.get_unchecked(pointer.0 as usize).2 }
    }

    #[inline]
    pub(crate) fn pointers_of(&self, pointer: Pointer) -> PointerPair {
        unsafe {
            let pointers = self.nodes.get_unchecked(pointer.0 as usize).3;

            // This actually helps a bit! (5%?)
            let (low, high) = pointers.unpack();
            let low_ref: *const (u32, u16, Variable, PointerPair) = self.nodes.get_unchecked(low.0 as usize);
            std::arch::x86_64::_mm_prefetch::<3>(low_ref as (*const i8));
            let high_ref: *const (u32, u16, Variable, PointerPair) = self.nodes.get_unchecked(high.0 as usize);
            std::arch::x86_64::_mm_prefetch::<3>(high_ref as (*const i8));

            pointers
        }
    }

    pub(crate) fn root_pointer(&self) -> Pointer {
        Pointer((self.nodes.len() - 1) as u32)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

}

impl Default for Bdd {
    fn default() -> Self {
        Bdd::new_false()
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
            if node_items.next().is_some() || variable.is_none() || left_pointer.is_none() || right_pointer.is_none() {
                return Err(format!("Unexpected node representation `{}`.", node_string));
            }
            let variable = if let Ok(x) = variable.unwrap().parse::<u16>() { x } else {
                return Err(format!("Invalid variable numeral `{}`.", variable.unwrap()));
            };
            let left_pointer = if let Ok(x) = left_pointer.unwrap().parse::<u32>() { x } else {
                return Err(format!("Invalid pointer numeral `{}`.", left_pointer.unwrap()))
            };
            let right_pointer = if let Ok(x) = right_pointer.unwrap().parse::<u32>() { x } else {
                return Err(format!("Invalid pointer numeral `{}`.", right_pointer.unwrap()))
            };
            //node_variables.push(Variable(variable));
            //node_pointers.push(Pointer(left_pointer) | Pointer(right_pointer));
            nodes.push((0,0,Variable(variable), Pointer(left_pointer) | Pointer(right_pointer)));
        }
        Ok(Bdd { nodes })
    }

}