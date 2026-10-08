//! Paired exact retrieval microbenchmark, not collection throughput evidence.
use rrrah_dedup::{gradient, local};
use gradient::{GradientCellRecipe, GradientDescriptor};
#[path = "../../../docs/research/dedup-gradient-index-before-borrowed-distance.rs"]
mod owned;
#[path = "../src/gradient_index.rs"]
mod borrowed;
fn descriptor(mut state: u64) -> GradientDescriptor {
    let mut d = [0.; 128];
    for v in &mut d {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        *v = (state >> 32) as f64 + 1.;
    }
    let norm = d.iter().map(|v| v * v).sum::<f64>().sqrt();
    for v in &mut d { *v /= norm; }
    GradientDescriptor(d)
}
fn main() {
    let entries: Vec<_> = (1..=4096).map(|i| (i, descriptor(i))).collect();
    let queries: Vec<_> = (0..64).map(|i| descriptor(9000 + i)).collect();
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let recipe = GradientCellRecipe::Interpolated;
    let old = owned::GradientDescriptorIndex::new(&entries, recipe, 4096, &budget, || false).unwrap();
    let new = borrowed::GradientDescriptorIndex::new(&entries, recipe, 4096, &budget, || false).unwrap();
    // Same search parameters; compare every retained ID and exact f64 distance.
    for round in 0..6 {
        let mut results = Vec::new();
        for backend in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let start = std::time::Instant::now();
            let values: Vec<Vec<(u64, f64)>> = queries.iter().map(|query| {
                if backend == 0 {
                    old.search(query, recipe, 0.1, 4096, 4096, || false).unwrap()
                        .iter().map(|v| (v.id, v.squared_distance)).collect()
                } else {
                    new.search(query, recipe, 0.1, 4096, 4096, || false).unwrap()
                        .iter().map(|v| (v.id, v.squared_distance)).collect()
                }
            }).collect();
            let seconds = start.elapsed().as_secs_f64();
            println!("{{\"round\":{round},\"backend\":{backend},\"seconds\":{seconds},\"queries\":64,\"entries\":4096}}");
            results.push(values);
        }
        assert_eq!(results[0], results[1]);
    }
    drop(old); drop(new);
    assert_eq!(budget.used(), 0);
    println!("{{\"status\":\"complete_exact_paired_retrieval\",\"memory_used\":0}}");
}
