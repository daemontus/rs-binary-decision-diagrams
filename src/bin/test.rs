//use binary_decision_diagrams::v2::Bdd;
use binary_decision_diagrams::v3::core::bdd::Bdd;
use std::convert::TryFrom;
use std::time::SystemTime;

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
        let left_path = format!("./bench_inputs/itgr/{}.and_not.left.bdd", benchmark);
        let mut left =
            Bdd::try_from(std::fs::read_to_string(&left_path).unwrap().as_str()).unwrap();
        let left = left.sort_preorder();
        let right_path = format!("./bench_inputs/itgr/{}.and_not.right.bdd", benchmark);
        let mut right =
            Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
        right = right.sort_preorder();
        if left.node_count() == 326271 {
            let mut k = 0;
            let start = SystemTime::now();
            for _ in 0..1 {
                //k += left.and(&right).node_count();
                k += binary_decision_diagrams::v3::core::ooo::apply(&left, &right).node_count();
                //k += and_not_u48_function(&left, &right).node_count();
                //k += gen_tasks(&left, &right, &mut task_cache, &mut node_cache);
            }
            println!(
                "Just for fun {} - Elapsed: {}",
                k,
                start.elapsed().unwrap().as_millis()
            );
        }
        //println!("{} {}: {}", node_count, benchmark, result.node_count());
    }
}
