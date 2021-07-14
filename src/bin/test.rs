use binary_decision_diagrams::Bdd;
use binary_decision_diagrams::_bdd_u32::PartialNodeCache;
use binary_decision_diagrams::_bdd_u32::_impl_task_bench::{gen_tasks, TaskCache, UnrolledStack};
use std::convert::TryFrom;

fn main() {
    let mut benchmarks = Vec::new();
    for file in std::fs::read_dir("./bench_inputs/itgr").unwrap() {
        let file = file.unwrap();
        let path = file.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with(".and_not.left.bdd") {
            let bench_name = &file_name[..(file_name.len() - 17)];
            benchmarks.push(bench_name.to_string());
        }
    }
    // Actually do the benchmarks in some sensible order.
    benchmarks.sort_by_cached_key(|name| {
        let mut split = name.split(".");
        split.next();
        let size = split.next().unwrap();
        size.parse::<usize>().unwrap()
    });

    for benchmark in &benchmarks {
        let mut split = benchmark.split(".");
        split.next();
        let size = split.next().unwrap();
        let node_count = size.parse::<usize>().unwrap();
        let left_path = format!("./bench_inputs/itgr/{}.and_not.left.bdd", benchmark);
        let left = Bdd::try_from(std::fs::read_to_string(&left_path).unwrap().as_str()).unwrap();
        let right_path = format!("./bench_inputs/itgr/{}.and_not.right.bdd", benchmark);
        let right = Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
        if left.node_count() == 326271 {
            let mut task_cache = TaskCache::new(326271);
            //let mut stack = UnrolledStack::new(5000);
            let mut node_cache = PartialNodeCache::new(2 * 326271);
            println!(
                "Task count: {}",
                gen_tasks(&left, &right, &mut task_cache, &mut node_cache)
            );
            let mut k = 0;
            for _ in 0..1000 {
                k += gen_tasks(&left, &right, &mut task_cache, &mut node_cache);
            }
            println!("Just for fun {}", k);
        }
        //println!("{} {}: {}", node_count, benchmark, result.node_count());
    }
}
