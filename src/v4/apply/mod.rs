pub mod task_cache;

use super::core::Bdd;
use task_cache::TaskCache;

pub fn apply(left_bdd: &Bdd, right_bdd: &Bdd) -> Bdd {
    let mut _task_cache = TaskCache::new(left_bdd, right_bdd);
    todo!()
}