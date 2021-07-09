use crate::_bdd_u32::{CompleteTaskQueue, Task};
use crate::{Variable, Pointer};

impl CompleteTaskQueue {

    pub fn new_empty() -> CompleteTaskQueue {
        CompleteTaskQueue {
            list_roots: vec![0; (u16::MAX as usize)],
            tasks: {
                let mut x = Vec::with_capacity(1024);
                x.push(Task {
                    dependencies: (0,0),
                    result: Pointer::zero(),
                    next_task: 0,
                });
                x.push(Task {
                    dependencies: (1,1),
                    result:Pointer::one(),
                    next_task: 1,
                });
                x
            }
        }
    }

    pub fn reset(&mut self, variables: u16) {
        for i_v in 0..variables {
            self.list_roots[i_v as usize] = 0;
        }
        unsafe {    // A bit dangerous, but essentially erases all allocated tasks without freeing them (which is ok since they are all copy types anyway).
            self.tasks.set_len(2);
        }
    }

    pub fn new(variables: u16, expected_capacity: usize) -> CompleteTaskQueue {
        CompleteTaskQueue {
            list_roots: vec![0; usize::from(variables)],
            tasks: {
                // Item zero and one are reserved as root tasks (plus zero also servers as list end)
                let mut x = Vec::with_capacity(expected_capacity);
                x.push(Task {
                    dependencies: (0,0),
                    result: Pointer::zero(),
                    next_task: 0,
                });
                x.push(Task {
                    dependencies: (1,1),
                    result: Pointer::one(),
                    next_task: 1,
                });
                x
            },
        }
    }

    #[inline]
    pub fn reserve_task(&mut self, variable: Variable) -> usize {
        let variable = usize::from(variable.0);
        let list_root = unsafe { self.list_roots.get_unchecked_mut(variable) };
        self.tasks.push(Task {
            dependencies: (0,0),
            result: Pointer::undef(),
            next_task: *list_root,
        });
        let index = self.tasks.len() - 1;
        *list_root = index;
        index
    }

    #[inline]
    pub fn set_dependencies(&mut self, task: usize, dependencies: (usize, usize)) {
        unsafe {
            let cell = self.tasks.get_unchecked_mut(task);
            let dep_cell = &mut cell.dependencies;
            *dep_cell = dependencies;
        }
    }

    #[inline]
    pub fn variable_iteration(&self, variable: Variable) -> usize {
        unsafe { *self.list_roots.get_unchecked(usize::from(variable.0)) }
    }

    #[inline]
    pub fn get(&self, index: usize) -> &Task {
        unsafe { self.tasks.get_unchecked(index) }
    }

}