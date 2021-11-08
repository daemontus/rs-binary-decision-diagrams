use binary_decision_diagrams::perf_testing::bdd::Bdd;
use std::convert::TryFrom;
use perfcnt::linux::{PerfCounterBuilderLinux, HardwareEventType};
use criterion::measurement::Measurement;
use criterion_perf_events::Perf;
use binary_decision_diagrams::perf_testing::naive_coupled_dfs::naive_coupled_dfs;

fn new_cpu_cycles_counter() -> Perf {
    criterion_perf_events::Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CPUCycles))
}

fn new_instructions_counter() -> Perf {
    criterion_perf_events::Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::Instructions))
}

fn new_cache_reference_counter() -> Perf {
    criterion_perf_events::Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CacheReferences))
}

fn new_cache_miss_counter() -> Perf {
    criterion_perf_events::Perf::new(PerfCounterBuilderLinux::from_hardware_event(HardwareEventType::CacheMisses))
}

fn main() {
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

    for benchmark in benchmarks {
        println!("Benchmark {}", benchmark);

        let left_path = format!("./bench_inputs/reach/{}.or.left.bdd", benchmark);
        let right_path = format!("./bench_inputs/reach/{}.or.right.bdd", benchmark);

        let left = std::fs::read_to_string(&left_path)
            .ok()
            .and_then(|it| Bdd::try_from(it.as_str()).ok())
            .unwrap();
        println!("Left ready: {}", left.node_count());
        let right = std::fs::read_to_string(&right_path)
            .ok()
            .and_then(|it| Bdd::try_from(it.as_str()).ok())
            .unwrap();
        println!("Right ready: {}", right.node_count());

        let left = left.sort_postorder();
        let right = right.sort_postorder();

        println!("warmup run...");
        benchmark_code(&left, &right);

        let cycles = new_cpu_cycles_counter();
        let instructions = new_instructions_counter();
        let cache_references = new_cache_reference_counter();
        let cache_misses = new_cache_miss_counter();

        let i_cycles = cycles.start();
        let i_instructions = instructions.start();
        let i_cache_references = cache_references.start();
        let i_cache_misses = cache_misses.start();

        let product_nodes = benchmark_code(&left, &right);

        let cycles = cycles.end(i_cycles);
        let instructions = instructions.end(i_instructions);
        let cache_references = cache_references.end(i_cache_references);
        let cache_misses = cache_misses.end(i_cache_misses);
        let ipc = (instructions as f64) / (cycles as f64);
        let hit_rate = 100.0 - (100.0 * (cache_misses as f64) / (cache_references as f64));
        let instructions_per_node = (instructions as f64) / (product_nodes as f64);
        let cycles_per_node = (cycles as f64) / (product_nodes as f64);

        println!("| {} | {} | {} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} |",
                 benchmark,
                 product_nodes,
                 cycles,
                 instructions,
                 cache_references,
                 cache_misses,
                 ipc,
                 hit_rate,
                 instructions_per_node,
                 cycles_per_node,
        )

    }
}

fn benchmark_code(left: &Bdd, right: &Bdd) -> usize {
    let counted = naive_coupled_dfs(left, right);
    println!("Counted {} nodes.", counted);
    counted
}