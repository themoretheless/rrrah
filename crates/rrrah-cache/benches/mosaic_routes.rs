//! Dependency-free microbenchmark for the decoded-mosaic cache routes.
//!
//! Compares the three foreground load routes for a realistic synthetic mosaic
//! (24 MP, 6000x4000, 1 component, ~45.8 MiB of u16 pixels):
//!
//!   * `ram_route`   — `MosaicRamCache` hit: promoting get + visible pin churn,
//!   * `disk_route`  — `SourceFingerprint` + `DiskMosaicCache::load` incl.
//!     BLAKE3 payload verification,
//!   * `navigation`  — back/forward browsing across a frame window with the
//!     RAM cache enabled vs disabled (disk-only fallback, the `--ram_cache_mb 0`
//!     configuration),
//!   * `eviction`    — pin protection under budget pressure: the visible frame
//!     must survive eviction storms and lookups must stay fast.
//!
//! A synthetic cold decode is not benchmarked here: there is no synthetic RAW
//! encoder in this workspace, and a constant-filled stand-in would misrepresent
//! entropy-decode cost. Real decode timings live in `docs/DNG_BENCHMARK_*`.
//!
//! Run with:
//!
//! `cargo bench -p rrrah-cache --bench mosaic_routes`
//!
//! Measurements are informational; there is intentionally no wall-clock gate.
//! Workload controls: `RRRAH_MOSAIC_BENCH_FRAMES`, `RRRAH_MOSAIC_BENCH_SAMPLES`,
//! `RRRAH_MOSAIC_BENCH_NAV_STEPS`.

#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

use rrrah_cache::{CacheKey, DiskMosaicCache, MosaicRamCache, SourceFingerprint};
use rrrah_core::{
    CfaColor, CfaPattern, DecodedMosaic, LevelGrid, MosaicRecipeManifest, Orientation, Photometric,
    REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS, RawMetadata, WhiteLevel,
};

const MOSAIC_WIDTH: u32 = 6000;
const MOSAIC_HEIGHT: u32 = 4000;
const DEFAULT_FRAMES: usize = 8;
const DEFAULT_SAMPLES: usize = 30;
const DEFAULT_NAV_STEPS: usize = 200;
/// Synthetic stand-in for a RAW source file; only its sampled bytes and
/// metadata feed the fingerprint, so 8 MiB is representative of the sampled
/// fingerprint cost without the write time of a full 30 MB file.
const SOURCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct SampleStats {
    min: f64,
    p50: f64,
    p95: f64,
}

struct Fixture {
    dir: tempfile::TempDir,
    keys: Vec<CacheKey>,
    mosaics: Vec<DecodedMosaic>,
    cache: DiskMosaicCache,
}

fn main() {
    let frames = positive_env("RRRAH_MOSAIC_BENCH_FRAMES", DEFAULT_FRAMES);
    let samples = positive_env("RRRAH_MOSAIC_BENCH_SAMPLES", DEFAULT_SAMPLES);
    let nav_steps = positive_env("RRRAH_MOSAIC_BENCH_NAV_STEPS", DEFAULT_NAV_STEPS);
    assert!(frames >= 2, "navigation benchmark needs at least two frames");

    println!("populating synthetic fixture: frames={frames} mosaic={MOSAIC_WIDTH}x{MOSAIC_HEIGHT}");
    let populate_started = Instant::now();
    let fixture = Fixture::new(frames);
    let mosaic_mib = fixture.mosaics[0].byte_len() as f64 / (1024.0 * 1024.0);
    println!(
        "fixture_ready frames={} mosaic_mib={mosaic_mib:.2} populate_s={:.2}",
        fixture.mosaics.len(),
        populate_started.elapsed().as_secs_f64(),
    );

    ram_route(&fixture, samples);
    disk_route(&fixture, samples);
    navigation(&fixture, nav_steps, samples);
    eviction_under_pressure(samples);
}

/// Route (a): RAM hit. Each operation is one navigation step on the loader
/// thread: promoting get followed by the visible-frame pin handoff.
fn ram_route(fixture: &Fixture, samples: usize) {
    let capacity =
        u64::try_from(fixture.mosaics[0].byte_len() * fixture.mosaics.len() * 2).expect("capacity fits u64");
    let mut ram = MosaicRamCache::new(capacity);
    for (key, mosaic) in fixture.keys.iter().zip(&fixture.mosaics) {
        assert!(ram.insert(*key, mosaic.clone()));
    }
    // Warm the clock/cache lines before sampling.
    for key in &fixture.keys {
        black_box(ram.get(black_box(key)));
        ram.mark_visible(key);
    }
    let stats = measure(samples, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        // Forward sweep then backward sweep, as arrow-key browsing does.
        for key in fixture.keys.iter().chain(fixture.keys.iter().rev()) {
            let mosaic = ram.get(black_box(key)).expect("resident frame");
            ram.mark_visible(key);
            sink ^= u64::from(mosaic.pixels[0]);
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });
    let ops = fixture.keys.len() * 2;
    println!(
        "ram_route ops_per_sample={ops} samples={samples} p50_ns_op={:.2} p95_ns_op={:.2} best_ns_op={:.2}",
        ns_per_operation(stats.p50, ops),
        ns_per_operation(stats.p95, ops),
        ns_per_operation(stats.min, ops),
    );
}

/// Route (b): disk hit. Each operation is the full production disk route:
/// fingerprint the source, derive the key, load and BLAKE3-verify the mosaic.
fn disk_route(fixture: &Fixture, samples: usize) {
    let sources = fixture.source_paths();
    let recipe = benchmark_manifest();
    // One warm load per key so OS pages and the decoder tables are hot.
    for (index, key) in fixture.keys.iter().enumerate() {
        let fingerprint = SourceFingerprint::from_path(&sources[index]).expect("fingerprint");
        assert_eq!(
            CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe),
            *key,
            "fixture keys must reproduce from the synthetic sources"
        );
        black_box(fixture.cache.load(black_box(*key)).expect("warm load"));
    }
    let stats = measure(samples, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for (index, _) in fixture.keys.iter().enumerate() {
            let fingerprint = SourceFingerprint::from_path(black_box(&sources[index])).expect("fingerprint");
            let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
            let hit = fixture
                .cache
                .load(black_box(key))
                .expect("disk load")
                .expect("resident on disk");
            sink ^= u64::from(hit.mosaic.pixels[0]);
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });
    let ops = fixture.keys.len();
    println!(
        "disk_route ops_per_sample={ops} samples={samples} p50_ms_op={:.3} p95_ms_op={:.3} best_ms_op={:.3}",
        ns_per_operation(stats.p50, ops) / 1e6,
        ns_per_operation(stats.p95, ops) / 1e6,
        ns_per_operation(stats.min, ops) / 1e6,
    );
}

/// Back/forward navigation across the frame window. `ram` models the shipped
/// configuration; `disk_only` models `--ram_cache_mb 0`, where every step pays
/// the disk route. Reports per-step wall time for one full sweep.
fn navigation(fixture: &Fixture, nav_steps: usize, samples: usize) {
    let capacity =
        u64::try_from(fixture.mosaics[0].byte_len() * fixture.mosaics.len() * 2).expect("capacity fits u64");
    let mut ram = MosaicRamCache::new(capacity);
    for (key, mosaic) in fixture.keys.iter().zip(&fixture.mosaics) {
        assert!(ram.insert(*key, mosaic.clone()));
    }
    let order = navigation_order(fixture.keys.len(), nav_steps);

    let ram_stats = measure(samples, || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for &index in black_box(&order) {
            let key = fixture.keys[index];
            let mosaic = ram.get(black_box(&key)).expect("resident frame");
            ram.mark_visible(&key);
            sink ^= u64::from(mosaic.pixels[0]);
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });

    let sources = fixture.source_paths();
    let recipe = benchmark_manifest();
    let disk_stats = measure(samples.min(5), || {
        let started = Instant::now();
        let mut sink = 0_u64;
        for &index in black_box(&order) {
            let fingerprint = SourceFingerprint::from_path(black_box(&sources[index])).expect("fingerprint");
            let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, recipe);
            let hit = fixture
                .cache
                .load(black_box(key))
                .expect("disk load")
                .expect("resident on disk");
            sink ^= u64::from(hit.mosaic.pixels[0]);
        }
        black_box(sink);
        started.elapsed().as_secs_f64()
    });

    println!(
        "navigation steps={} frames={} ram_p50_us_step={:.2} ram_p95_us_step={:.2} disk_only_p50_ms_step={:.3} disk_only_p95_ms_step={:.3} speedup_p50={:.0}x",
        order.len(),
        fixture.keys.len(),
        ns_per_operation(ram_stats.p50, order.len()) / 1e3,
        ns_per_operation(ram_stats.p95, order.len()) / 1e3,
        ns_per_operation(disk_stats.p50, order.len()) / 1e6,
        ns_per_operation(disk_stats.p95, order.len()) / 1e6,
        disk_stats.p50 / ram_stats.p50,
    );
}

/// Eviction under pressure: budget holds three frames, the visible frame is
/// pinned, and an eviction storm of oversized admissions follows. The visible
/// frame must survive every round, and lookups of it are timed while the
/// cache is over budget.
fn eviction_under_pressure(samples: usize) {
    let mosaic = synthetic_mosaic(0);
    let frame_bytes = mosaic.byte_len();
    let budget = u64::try_from(frame_bytes * 3).expect("budget fits u64");
    let mut ram = MosaicRamCache::new(budget);
    let visible = CacheKey::for_mosaic_recipe(&fingerprint_for(0), 0, benchmark_manifest());
    assert!(ram.insert(visible, mosaic.clone()));
    ram.mark_visible(&visible);

    let storm_keys: Vec<CacheKey> = (1..=64)
        .map(|index| CacheKey::for_mosaic_recipe(&fingerprint_for(index), 0, benchmark_manifest()))
        .collect();
    // Pixel generation is outside the timed region; admission itself is an
    // Arc-cheap clone plus byte-accounted eviction.
    let storm_mosaic = synthetic_mosaic(1);
    let mut insert_ns = Vec::new();
    let mut lookup_ns = Vec::new();
    for key in &storm_keys {
        let started = Instant::now();
        // Each insert must evict roughly one unpinned frame to fit.
        black_box(ram.insert(black_box(*key), storm_mosaic.clone()));
        insert_ns.push(started.elapsed());
        let started = Instant::now();
        let hit = ram.get(black_box(&visible)).expect("visible frame survives");
        black_box(u64::from(hit.pixels[0]));
        lookup_ns.push(started.elapsed());
        // Keep the storm pinned on the same visible frame.
        ram.mark_visible(&visible);
        assert!(
            ram.resident_weight() <= budget,
            "resident bytes must stay within budget under pressure"
        );
    }
    assert!(ram.get(&visible).is_some(), "pinned frame survived the storm");
    assert_eq!(ram.visible(), Some(visible));

    insert_ns.sort_by(Duration::cmp);
    lookup_ns.sort_by(Duration::cmp);
    println!(
        "eviction rounds={} budget_frames=3 insert_p50_us={:.2} insert_p95_us={:.2} visible_lookup_p50_ns={:.2} visible_lookup_p95_ns={:.2} visible_survived=true",
        storm_keys.len(),
        duration_us(insert_ns[insert_ns.len() / 2]),
        duration_us(insert_ns[insert_ns.len() * 95 / 100]),
        duration_ns(lookup_ns[lookup_ns.len() / 2]),
        duration_ns(lookup_ns[lookup_ns.len() * 95 / 100]),
    );
    // Quiet `samples` into the output so configuration drift is visible.
    println!("eviction samples_config={samples} (single deterministic run)");
}

impl Fixture {
    fn new(frames: usize) -> Self {
        let dir = tempfile::tempdir().expect("fixture tempdir");
        let cache = DiskMosaicCache::new(dir.path().join("mosaics"));
        let mut keys = Vec::with_capacity(frames);
        let mut mosaics = Vec::with_capacity(frames);
        for index in 0..frames {
            let source = dir.path().join(format!("frame-{index}.synthetic-raw"));
            std::fs::write(&source, deterministic_bytes(SOURCE_BYTES, index as u64))
                .expect("write synthetic source");
            let fingerprint = SourceFingerprint::from_path(&source).expect("fingerprint");
            let key = CacheKey::for_mosaic_recipe(&fingerprint, 0, benchmark_manifest());
            let mosaic = synthetic_mosaic(index as u64);
            cache.store(key, &mosaic).expect("store mosaic");
            keys.push(key);
            mosaics.push(mosaic);
        }
        Self {
            dir,
            keys,
            mosaics,
            cache,
        }
    }

    fn source_paths(&self) -> Vec<std::path::PathBuf> {
        (0..self.keys.len())
            .map(|index| self.dir.path().join(format!("frame-{index}.synthetic-raw")))
            .collect()
    }
}

/// Alternate forward and backward sweeps across the window, the access
/// pattern of arrow-key browsing that reverses direction.
fn navigation_order(frames: usize, steps: usize) -> Vec<usize> {
    let mut order = Vec::with_capacity(steps);
    let mut index = 0_usize;
    let mut forward = true;
    for _ in 0..steps {
        order.push(index);
        if forward {
            if index + 1 < frames {
                index += 1;
            } else {
                forward = false;
            }
        } else if index > 0 {
            index -= 1;
        } else {
            forward = true;
        }
    }
    order
}

fn synthetic_mosaic(seed: u64) -> DecodedMosaic {
    let pixels = usize::try_from(MOSAIC_WIDTH)
        .and_then(|w| usize::try_from(MOSAIC_HEIGHT).map(|h| w * h))
        .expect("mosaic dimensions fit usize");
    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ seed;
    let pixels: Vec<u16> = (0..pixels)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u16 & 0x3fff
        })
        .collect();
    DecodedMosaic::new(
        RawMetadata {
            make: "Synthetic".into(),
            model: "Bench".into(),
            width: MOSAIC_WIDTH,
            height: MOSAIC_HEIGHT,
            components_per_pixel: 1,
            bits_per_sample: 14,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![512.0],
            },
            white_level: WhiteLevel(vec![16_383.0]),
            white_balance: [1.0, 1.0, 1.0, 1.0],
            xyz_to_camera: [[0.0; 3]; 4],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        },
        Arc::new(pixels),
    )
    .expect("synthetic mosaic is valid")
}

fn fingerprint_for(seed: u64) -> SourceFingerprint {
    let mut sampled = [0_u8; 32];
    sampled[..8].copy_from_slice(&seed.to_le_bytes());
    SourceFingerprint {
        file_size: SOURCE_BYTES as u64,
        modified_ns: u128::from(seed),
        sampled_blake3: sampled,
    }
}

fn benchmark_manifest() -> MosaicRecipeManifest {
    MosaicRecipeManifest::new(1, 1, 1, 1, REQUIRED_SENSOR_MOSAIC_DECODE_FLAGS, [0x5a; 32])
}

fn deterministic_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = 0x52_52_52_41_48_4b_45_59_u64 ^ seed.wrapping_mul(0x9e37_79b9);
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
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

fn duration_us(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

fn duration_ns(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e9
}
