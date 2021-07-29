use binary_decision_diagrams::v2::bench_fun::deps::{Bdd, NodeId};
use criterion::{criterion_group, criterion_main, Criterion};
use std::convert::TryFrom;
//use binary_decision_diagrams::_bdd_u32::_impl_task_bench::{gen_tasks, TaskCache, UnrolledStack};
//use criterion::measurement::WallTime;
use criterion_perf_events::Perf;
use perfcnt::linux::{HardwareEventType, PerfCounterBuilderLinux};
use binary_decision_diagrams::v2::bench_fun::{explore, apply, naive_coupled_dfs, optimized_coupled_dfs};
use std::process::exit;
use biodivine_lib_bdd::Bdd as LibBdd;
use biodivine_lib_bdd::BddVariableSet;
use cudd_sys::{Cudd_Init, CUDD_UNIQUE_SLOTS, CUDD_CACHE_SLOTS, Cudd_Quit, Cudd_bddOr, Cudd_DagSize, Cudd_DisableGarbageCollection};
use criterion::measurement::Measurement;
//use binary_decision_diagrams::_bdd_u32::PartialNodeCache;

pub fn criterion_benchmark(c: &mut Criterion<Perf>) {
    let mut benchmarks = Vec::new();
    for file in std::fs::read_dir("./bench_inputs/reach").unwrap() {
        let file = file.unwrap();
        let path = file.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with(".or.left.bdd") {
            let bench_name = &file_name[..(file_name.len() - ".or.left.bdd".len())];
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

    let mut group = c.benchmark_group("itgr");
    group.sample_size(10);
    // Some of the benchmarks will run for a long time, so this setting is recommended.
    //group.sampling_mode(SamplingMode::Flat);
    for benchmark in &benchmarks {
        let left_path = format!("./bench_inputs/reach/{}.or.left.bdd", benchmark);
        println!("Left path: {} {}", left_path, benchmark);
        let mut left =
            Bdd::try_from(std::fs::read_to_string(&left_path).unwrap().as_str()).unwrap();
        println!("Left ready: {}", left.node_count());
        left.sort_preorder_safe();
        let right_path = format!("./bench_inputs/reach/{}.or.right.bdd", benchmark);
        let mut right =
            Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
        println!("Right ready: {}", right.node_count());
        right.sort_preorder_safe();

        println!("Task count: {} (minimal)", naive_coupled_dfs(&left, &right));
        println!("Node count: {} (actual)", apply(&left, &right).node_count());

        //let left = LibBdd::from_string(std::fs::read_to_string(&left_path).unwrap().as_str());
        //let right = LibBdd::from_string(std::fs::read_to_string(&right_path).unwrap().as_str());

        //println!("Size: {}", left.or(&right).node_count());

        println!("Translating...");
        let cudd = unsafe { Cudd_Init(0, 0, 1_000_000, 1_000_000, 0) };
        unsafe { Cudd_DisableGarbageCollection(cudd); }
        let dd_left = left.move_to_cudd(cudd);
        let dd_right = right.move_to_cudd(cudd);
        let left_nodes = unsafe { Cudd_DagSize(dd_left) };
        println!("Nodes (left): {}", left_nodes);
        let measurement = Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::Instructions));
        let start = measurement.start();
        let result = unsafe { Cudd_bddOr(cudd, dd_left, dd_right) };
        let end = measurement.end(start);
        let result_nodes = unsafe { Cudd_DagSize(result) };
        println!("Measurement: {}, result: {}", end, result_nodes);
        unsafe { Cudd_Quit(cudd); }

        group.bench_function(benchmark, |b| {
            /*println!("Translating...");
            let cudd = unsafe { Cudd_Init(0, 0, CUDD_UNIQUE_SLOTS, CUDD_CACHE_SLOTS, 0) };
            let dd_left = left.move_to_cudd(cudd);
            let dd_right = right.move_to_cudd(cudd);
            let left_nodes = unsafe { Cudd_DagSize(dd_left) };
            println!("Nodes (left): {}", left_nodes);
            let measurement = Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::Instructions));
            let start = measurement.start();
            let result = unsafe { Cudd_bddOr(cudd, dd_left, dd_right) };
            let end = measurement.end(start);
            let result_nodes = unsafe { Cudd_DagSize(result) };
            println!("Instructions: {}, result: {}", end, result_nodes);*/
            b.iter(|| {
                //unsafe { Cudd_bddOr(cudd, dd_left, dd_right) }
                //left.or(&right)
                //left.or(&right)
                apply(&left, &right).node_count()
                //optimized_coupled_dfs(&left, &right)
                //explore(&left)
                //exit(128)
            });
        });

        //unsafe { Cudd_Quit(cudd); }
    }
    group.finish();
}


criterion_group!(
    name = benches;
    config = Criterion::default().with_measurement(Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::Instructions)));
    targets = criterion_benchmark
);


//criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
