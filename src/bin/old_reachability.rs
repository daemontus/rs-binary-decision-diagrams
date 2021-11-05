use std::io::Read;
use biodivine_lib_param_bn::symbolic_async_graph::SymbolicAsyncGraph;
use biodivine_lib_param_bn::BooleanNetwork;
use std::convert::TryFrom;
use biodivine_lib_param_bn::biodivine_std::traits::Set;
use std::time::SystemTime;

fn main() {
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer).unwrap();
    let model = BooleanNetwork::try_from(buffer.as_str()).unwrap();

    let graph = SymbolicAsyncGraph::new(model.clone()).unwrap();

    let start = SystemTime::now();
    let mut universe = graph.mk_unit_colored_vertices();

    while !universe.is_empty() {
        println!("Universe size: {}, cardinality: {}", universe.as_bdd().size(), universe.approx_cardinality());
        let mut reachable = universe.pick_vertex();

        let mut i = 0;
        loop {
            i += 1;
            let mut done = true;

            for v in model.variables() {
                let successors = graph.var_post(v, &reachable).minus(&reachable).intersect(&universe);

                if !successors.is_empty() {
                    done = false;
                    reachable = reachable.union(&successors);
                    let elapsed = start.elapsed().unwrap().as_millis();
                    println!("({}) Iteration ({}), reach size: {}", elapsed, i, reachable.as_bdd().size());
                }
            }

            println!("Iteration ({}), reach size: {}", i, reachable.as_bdd().size());

            if done {
                break;
            }
        }

        universe = universe.minus(&reachable);
    }
}