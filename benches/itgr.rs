use binary_decision_diagrams::v2::Bdd;
use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use std::convert::TryFrom;
//use binary_decision_diagrams::_bdd_u32::_impl_task_bench::{gen_tasks, TaskCache, UnrolledStack};
//use criterion::measurement::WallTime;
use criterion_perf_events::Perf;
use perfcnt::linux::{HardwareEventType, PerfCounterBuilderLinux};
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
        left.sort_preorder();
        println!("Left ready");
        let right_path = format!("./bench_inputs/reach/{}.or.right.bdd", benchmark);
        let mut right =
            Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
        right.sort_preorder();
        println!("Right ready");
        //if left.node_count() == 326271 {
            println!(
                "Node count: {}",
                left.or(&right).node_count()
            );
            group.bench_function(benchmark, |b| {
                b.iter(|| {
                    left.or(&right)
                });
            });
        //}
    }
    group.finish();
}


criterion_group!(
    name = benches;
    config = Criterion::default().with_measurement(Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CPUCycles)));
    targets = criterion_benchmark
);


//criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
