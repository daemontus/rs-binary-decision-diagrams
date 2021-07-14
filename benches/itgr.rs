use binary_decision_diagrams::v2::{Bdd, NodeId};
use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use std::convert::TryFrom;
//use binary_decision_diagrams::_bdd_u32::_impl_task_bench::{gen_tasks, TaskCache, UnrolledStack};
use criterion::measurement::WallTime;
use criterion_perf_events::Perf;
use perfcnt::linux::{HardwareEventType, PerfCounterBuilderLinux};
//use binary_decision_diagrams::_bdd_u32::PartialNodeCache;

pub fn criterion_benchmark(c: &mut Criterion<Perf>) {
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

    let mut group = c.benchmark_group("itgr");
    // Some of the benchmarks will run for a long time, so this setting is recommended.
    //group.sampling_mode(SamplingMode::Flat);
    for benchmark in &benchmarks {
        let left_path = format!("./bench_inputs/itgr/{}.and_not.left.bdd", benchmark);
        let mut left = Bdd::try_from(std::fs::read_to_string(&left_path).unwrap().as_str()).unwrap();
        left.sort_preorder();
        let right_path = format!("./bench_inputs/itgr/{}.and_not.right.bdd", benchmark);
        let mut right = Bdd::try_from(std::fs::read_to_string(right_path).unwrap().as_str()).unwrap();
        right.sort_preorder();
        if left.node_count() == 326271 {
            //let mut task_cache = TaskCache::new(326270);
            //let mut stack = UnrolledStack::new(5000);
            //let mut node_cache = PartialNodeCache::new(326270);
            println!(
                "Node count: {}",
                //binary_decision_diagrams::v2::_impl_::bdd::apply::and_not(&left, &right)
                //    .node_count()
                left.and_not_u48(&right).node_count()
            );
            group.bench_function(/*"task-generator-326271"*/ benchmark, |b| {
                b.iter(|| {
                    //gen_tasks(&left, &right, &mut task_cache, &mut node_cache)
                    //binary_decision_diagrams::v2::_impl_::bdd::apply::and_not(&left, &right)
                    /*binary_decision_diagrams::v2::_impl_::bdd::binary_operations::u48::apply(&left, &right, |l, r| {
                        if l.is_zero() || r.is_one() {
                            NodeId::ZERO
                        } else if l.is_one() && r.is_zero() {
                            NodeId::ONE
                        } else {
                            NodeId::UNDEFINED
                        }
                    })*/
                    //binary_decision_diagrams::v2::_impl_::bdd::binary_operations::u48::and_not_u48_function(&left, &right);
                    left.and_not_u48(&right)
                    //left.and_not(&right)
                    //left.and_not_u48(&right)
                });
            });
        }
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
