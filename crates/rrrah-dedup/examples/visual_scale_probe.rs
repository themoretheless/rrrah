//! Reproducible descriptor-index scale probe with exhaustive result oracles.
use rrrah_dedup::{
    pixels::Fingerprint,
    visual::{VisualCandidate, VisualIndex},
};
use std::time::Instant;
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn exhaustive(entries: &[(u64, Fingerprint)], query: &Fingerprint, radius: u32) -> Vec<VisualCandidate> {
    let mut matches = entries
        .iter()
        .filter_map(|(id, other)| {
            let evidence = query.compare(other, true);
            (evidence.distance <= radius).then_some(VisualCandidate { id: *id, evidence })
        })
        .collect::<Vec<_>>();
    matches.sort_unstable_by_key(|c| (c.evidence.distance, c.id));
    matches
}

fn main() {
    // Optional distribution and size select an isolated process for RSS measurement.
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    for distribution in ["random", "collision", "clustered"] {
        if args.first().is_some_and(|filter| filter != distribution) {
            continue;
        }
        for count in [1_000_usize, 10_000, 50_000] {
            if args.get(1).is_some_and(|filter| filter != &count.to_string()) {
                continue;
            }
            let mut state = 123_456_789_u64;
            let entries = (0..count)
                .map(|id| {
                    let mut variants = [[0; 4]; 8];
                    for variant in &mut variants {
                        for hash in variant {
                            let word = next(&mut state);
                            *hash = match distribution {
                                "collision" => {
                                    if id % 2 == 0 {
                                        0
                                    } else {
                                        u64::MAX
                                    }
                                }
                                "clustered" => {
                                    let base = if id % 2 == 0 { 0 } else { u64::MAX };
                                    base ^ (1_u64 << (word % 64)) ^ (1_u64 << ((word >> 8) % 64))
                                }
                                _ => word,
                            };
                        }
                    }
                    (
                        u64::try_from(id).unwrap(),
                        Fingerprint {
                            variants,
                            mean_linear_rgb: [0.5; 3],
                            luminance_stddev: 0.2,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let start = Instant::now();
            let index = VisualIndex::new(entries.iter().cloned(), count, || false).unwrap();
            let build = start.elapsed().as_secs_f64();
            for radius in [0, 8, 32, 128, 256] {
                // Two warmups, then four paired runs with alternating execution order.
                for _ in 0..2 {
                    let query = &entries[count / 3].1;
                    assert_eq!(
                        index.search(query, radius, true, || false).unwrap(),
                        exhaustive(&entries, query, radius)
                    );
                }
                let mut indexed = Vec::new();
                let mut brute = Vec::new();
                let mut hits = 0;
                for run in 0..4 {
                    let mut index_seconds = 0.0;
                    let mut exhaustive_seconds = 0.0;
                    let mut run_hits = 0;
                    for ordinal in [0, count / 3, count / 2, count - 1] {
                        let query = &entries[ordinal].1;
                        let mut actual = Vec::new();
                        let mut expected = Vec::new();
                        for operation in 0..2 {
                            let start = Instant::now();
                            if (run + operation) % 2 == 0 {
                                actual = index.search(query, radius, true, || false).unwrap();
                                index_seconds += start.elapsed().as_secs_f64();
                            } else {
                                expected = exhaustive(&entries, query, radius);
                                exhaustive_seconds += start.elapsed().as_secs_f64();
                            }
                        }
                        assert_eq!(
                            actual, expected,
                            "distribution={distribution} count={count} radius={radius}"
                        );
                        run_hits += actual.len();
                    }
                    indexed.push(index_seconds);
                    brute.push(exhaustive_seconds);
                    if run > 0 {
                        assert_eq!(hits, run_hits);
                    }
                    hits = run_hits;
                }
                println!(
                    "{{\"distribution\":\"{distribution}\",\"entries\":{count},\"radius\":{radius},\"queries_per_run\":4,\"runs\":4,\"warmups\":2,\"build_seconds\":{build},\"index_seconds\":{indexed:?},\"exhaustive_seconds\":{brute:?},\"hits_per_run\":{hits},\"oracle_equal\":true}}"
                );
            }
        }
    }
}
