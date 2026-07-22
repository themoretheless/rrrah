//! Dependency-free microbenchmark for the v3 cache identity/key primitives.
//!
//! This deliberately excludes file I/O: it measures the CPU work performed
//! after a stable-source resolver has supplied each input chunk. Run with:
//!
//! `cargo bench -p rrrah-cache --bench key_hashing --features bench-internals`
//!
//! Measurements are informational by default. Set `RRRAH_KEY_BENCH_GATE=1`
//! only on the calibrated performance runner to enable wall-clock thresholds.

#![allow(clippy::cast_precision_loss)]

use rrrah_cache::bench_support::{KeyFixture, hash_source, recipe_id};
use rrrah_core::{MosaicRecipeManifest, REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS};
use std::{hint::black_box, time::Instant};

const DEFAULT_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_SOURCE_ITERATIONS: usize = 8;
const DEFAULT_KEY_ITERATIONS: usize = 100_000;
const SOURCE_CHUNK_BYTES: usize = 256 * 1024;
const DEFAULT_SAMPLE_COUNT: usize = 30;

// Reference p95 thresholds for the calibrated performance runner. Shared CI
// reports measurements but never evaluates these wall-clock gates.
const DEFAULT_MIN_SOURCE_MIB_PER_SECOND: f64 = 512.0;
const DEFAULT_MAX_RECIPE_NS_PER_OPERATION: f64 = 1_500.0;
const DEFAULT_MAX_ARTIFACT_NS_PER_OPERATION: f64 = 2_000.0;

// Aspirational observability only; these do not fail the benchmark.
const TARGET_SOURCE_MIB_PER_SECOND: f64 = 1_024.0;
const TARGET_ARTIFACT_NS_PER_OPERATION: f64 = 1_000.0;

#[derive(Debug, Clone, Copy)]
struct SampleStats {
    min: f64,
    p50: f64,
    p95: f64,
}

#[derive(Debug, Clone, Copy)]
struct GateThresholds {
    min_source_mib_per_second: f64,
    max_recipe_ns_per_operation: f64,
    max_artifact_ns_per_operation: f64,
}

impl GateThresholds {
    fn from_env() -> Self {
        Self {
            min_source_mib_per_second: positive_f64_env(
                "RRRAH_KEY_BENCH_MIN_SOURCE_MIB_S",
                DEFAULT_MIN_SOURCE_MIB_PER_SECOND,
            ),
            max_recipe_ns_per_operation: positive_f64_env(
                "RRRAH_KEY_BENCH_MAX_RECIPE_NS_OP",
                DEFAULT_MAX_RECIPE_NS_PER_OPERATION,
            ),
            max_artifact_ns_per_operation: positive_f64_env(
                "RRRAH_KEY_BENCH_MAX_ARTIFACT_NS_OP",
                DEFAULT_MAX_ARTIFACT_NS_PER_OPERATION,
            ),
        }
    }
}

fn main() {
    let source_bytes = positive_env("RRRAH_KEY_BENCH_SOURCE_BYTES", DEFAULT_SOURCE_BYTES);
    let source_iterations = positive_env("RRRAH_KEY_BENCH_SOURCE_ITERS", DEFAULT_SOURCE_ITERATIONS);
    let key_iterations = positive_env("RRRAH_KEY_BENCH_KEY_ITERS", DEFAULT_KEY_ITERATIONS);
    let sample_count = positive_env("RRRAH_KEY_BENCH_SAMPLES", DEFAULT_SAMPLE_COUNT);
    let gate_enabled = flag_env("RRRAH_KEY_BENCH_GATE");
    let thresholds = GateThresholds::from_env();
    let source = deterministic_bytes(source_bytes);

    // Warm every path before collecting samples so one-time dispatch and page
    // faults are not attributed to the steady-state hash primitives.
    black_box(hash_source(&source, SOURCE_CHUNK_BYTES));
    let manifest = benchmark_manifest();
    black_box(recipe_id(manifest));
    let source_id = hash_source(&source, SOURCE_CHUNK_BYTES);
    let fixture = KeyFixture::new(source_id, manifest);
    black_box(fixture.artifact_key(0));

    let source_stats = measure(sample_count, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for _ in 0..source_iterations {
            sink ^= digest_prefix(&hash_source(black_box(&source), SOURCE_CHUNK_BYTES));
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });
    let recipe_stats = measure(sample_count, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for _ in 0..key_iterations {
            sink ^= digest_prefix(&recipe_id(black_box(manifest)));
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });
    let artifact_stats = measure(sample_count, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for image_index in 0..key_iterations {
            let image_index = u64::try_from(image_index).expect("benchmark iteration fits u64");
            sink ^= digest_prefix(&black_box(fixture.artifact_key(image_index)));
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });

    let source_total_bytes = source_bytes
        .checked_mul(source_iterations)
        .expect("benchmark byte count does not overflow usize");
    let source_mib = source_total_bytes as f64 / (1024.0 * 1024.0);
    let source_p50_mib_s = source_mib / source_stats.p50;
    let source_p95_mib_s = source_mib / source_stats.p95;
    let recipe_p50_ns = ns_per_operation(recipe_stats.p50, key_iterations);
    let recipe_p95_ns = ns_per_operation(recipe_stats.p95, key_iterations);
    let artifact_p50_ns = ns_per_operation(artifact_stats.p50, key_iterations);
    let artifact_p95_ns = ns_per_operation(artifact_stats.p95, key_iterations);

    println!(
        "source_id bytes={} chunk_bytes={SOURCE_CHUNK_BYTES} iterations={source_iterations} \
         samples={sample_count} p50_mib_s={source_p50_mib_s:.2} \
         p95_mib_s={source_p95_mib_s:.2} best_mib_s={:.2} gate_mib_s={:.2} \
         target_mib_s={TARGET_SOURCE_MIB_PER_SECOND:.2}",
        source_bytes,
        source_mib / source_stats.min,
        thresholds.min_source_mib_per_second,
    );
    println!(
        "recipe_id iterations={key_iterations} samples={sample_count} \
         p50_ns_op={recipe_p50_ns:.2} p95_ns_op={recipe_p95_ns:.2} \
         p50_mops={:.3} gate_max_ns_op={:.2}",
        operations_per_second(recipe_p50_ns) / 1_000_000.0,
        thresholds.max_recipe_ns_per_operation,
    );
    println!(
        "artifact_key iterations={key_iterations} samples={sample_count} \
         p50_ns_op={artifact_p50_ns:.2} p95_ns_op={artifact_p95_ns:.2} \
         p50_mops={:.3} gate_max_ns_op={:.2} \
         target_ns_op={TARGET_ARTIFACT_NS_PER_OPERATION:.2}",
        operations_per_second(artifact_p50_ns) / 1_000_000.0,
        thresholds.max_artifact_ns_per_operation,
    );

    println!(
        "performance_gate={}",
        if gate_enabled { "enabled" } else { "disabled" }
    );
    if gate_enabled {
        assert!(
            source_p95_mib_s >= thresholds.min_source_mib_per_second,
            "source-id p95 throughput regression: {source_p95_mib_s:.2} MiB/s < {:.2} MiB/s",
            thresholds.min_source_mib_per_second,
        );
        assert!(
            recipe_p95_ns <= thresholds.max_recipe_ns_per_operation,
            "recipe-id p95 latency regression: {recipe_p95_ns:.2} ns > {:.2} ns",
            thresholds.max_recipe_ns_per_operation,
        );
        assert!(
            artifact_p95_ns <= thresholds.max_artifact_ns_per_operation,
            "artifact-key p95 latency regression: {artifact_p95_ns:.2} ns > {:.2} ns",
            thresholds.max_artifact_ns_per_operation,
        );
    }
}

fn positive_env(name: &str, default: usize) -> usize {
    let value = match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive integer; got {value:?}")),
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("cannot read {name}: {error}"),
    };
    assert!(value > 0, "{name} must be positive");
    value
}

fn positive_f64_env(name: &str, default: f64) -> f64 {
    let value = match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive number; got {value:?}")),
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("cannot read {name}: {error}"),
    };
    assert!(
        value.is_finite() && value > 0.0,
        "{name} must be finite and positive"
    );
    value
}

fn flag_env(name: &str) -> bool {
    match std::env::var(name).as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("" | "0" | "false") => false,
        Ok("1" | "true") => true,
        Err(error) => panic!("cannot read {name}: {error}"),
        Ok(value) => panic!("{name} must be one of 0, 1, false or true; got {value:?}"),
    }
}

fn deterministic_bytes(len: usize) -> Vec<u8> {
    let mut state = 0x52_52_52_41_48_4b_45_59_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

fn benchmark_manifest() -> MosaicRecipeManifest {
    MosaicRecipeManifest::new(1, 1, 1, 1, REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS, [0x5a; 32])
}

fn digest_prefix(digest: &[u8; 32]) -> u64 {
    u64::from_le_bytes(digest[..8].try_into().expect("digest is 32 bytes"))
}

fn measure(mut samples: usize, mut operation: impl FnMut() -> f64) -> SampleStats {
    let mut values = Vec::with_capacity(samples);
    while samples > 0 {
        values.push(operation());
        samples -= 1;
    }
    values.sort_by(f64::total_cmp);
    SampleStats {
        min: values[0],
        p50: percentile(&values, 50),
        p95: percentile(&values, 95),
    }
}

fn percentile(sorted: &[f64], percentile: usize) -> f64 {
    let numerator = sorted.len().saturating_sub(1) * percentile;
    sorted[numerator.div_ceil(100)]
}

fn ns_per_operation(seconds: f64, operations: usize) -> f64 {
    seconds * 1_000_000_000.0 / operations as f64
}

fn operations_per_second(ns_per_operation: f64) -> f64 {
    1_000_000_000.0 / ns_per_operation
}
