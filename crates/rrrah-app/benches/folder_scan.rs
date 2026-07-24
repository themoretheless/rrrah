//! Synchronous `scan_folder` latency benchmark (task E).
//!
//! Hypothesis under test: calling `gallery::scan_folder` synchronously on the
//! winit UI thread (main.rs `open_dropped_path`, ~line 815) delays the first
//! frame past the 100 ms gate from `docs/GALLERY_ARCHITECTURE.md` ("first
//! catalog row, 1k entries | <=100 ms warm / <=250 ms cold") for 1k/10k-file
//! folders.
//!
//! The benchmark builds synthetic folders of 1k and 10k supported files
//! (plus junk sidecar files, as in a real photo directory). No RAW bytes are
//! needed: `scan_folder` only enumerates names and calls `symlink_metadata`
//! per entry. It measures the production `scan_folder` unchanged and also an
//! instrumented replica that splits the cost into enumerate+stat vs. sort.

#![allow(clippy::cast_precision_loss)]

#[allow(dead_code, unused_imports)]
#[path = "../src/decode_gate.rs"]
mod decode_gate;

#[allow(dead_code, unused_imports)]
#[path = "../src/cache_telemetry.rs"]
mod cache_telemetry;

#[allow(
    clippy::bool_to_int_with_if,
    clippy::cast_possible_truncation,
    clippy::collapsible_if,
    clippy::manual_ok_err,
    clippy::too_many_lines
)]
#[path = "../src/gallery.rs"]
mod gallery;

use gallery::{MAX_ITEMS, is_supported, scan_folder};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

/// Supported files per synthetic folder. Matches the two hypothesis sizes.
const SUPPORTED_COUNTS: [usize; 2] = [1_000, 10_000];
/// Extra non-supported files (JPG/XMP/TXT sidecars) per supported file, %.
const JUNK_PERCENT: usize = 25;

const WARMUP_RUNS: usize = 3;

fn iterations_for(count: usize) -> usize {
    if count >= 10_000 { 10 } else { 30 }
}

fn percentile(sorted_ns: &[u64], percentile: usize) -> u64 {
    let numerator = sorted_ns.len().saturating_sub(1) * percentile;
    sorted_ns[numerator.div_ceil(100)]
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// Create a folder with `supported` RAW-looking files and 25% junk files.
/// Filenames are zero-padded and shuffled deterministically so the directory
/// enumeration order does not come back pre-sorted.
fn build_folder(root: &Path, supported: usize) -> PathBuf {
    let folder = root.join(format!("folder_{supported}"));
    fs::create_dir_all(&folder).expect("create synthetic folder");
    let junk = supported * JUNK_PERCENT / 100;
    let total = supported + junk;
    for index in 0..total {
        // Deterministic shuffle: spreads extensions through enumeration order.
        let slot = index.wrapping_mul(65_537) % total.max(1);
        let name = if index % (100 + JUNK_PERCENT) < 100 {
            let extension = if index % 5 == 0 { "DNG" } else { "CR3" };
            format!("IMG_{slot:05}.{extension}")
        } else {
            let extension = match index % 3 {
                0 => "JPG",
                1 => "XMP",
                _ => "TXT",
            };
            format!("IMG_{slot:05}.{extension}")
        };
        fs::write(folder.join(name), b"").expect("write synthetic file");
    }
    folder
}

/// Instrumented replica of `gallery::scan_folder`, split into phases so the
/// report can attribute cost to enumeration+stat vs. the sort. Must stay in
/// sync with the production implementation; the benchmark asserts identical
/// output.
fn scan_folder_instrumented(folder: &Path) -> (Vec<PathBuf>, u64, u64) {
    let enumerate_started = Instant::now();
    let mut paths = fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    let metadata = fs::symlink_metadata(&path).ok()?;
                    (metadata.file_type().is_file() && is_supported(&path)).then_some(path)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let enumerate_ns = u64::try_from(enumerate_started.elapsed().as_nanos()).unwrap_or(u64::MAX);

    let sort_started = Instant::now();
    paths.sort_by_cached_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    paths.truncate(MAX_ITEMS);
    let sort_ns = u64::try_from(sort_started.elapsed().as_nanos()).unwrap_or(u64::MAX);

    (paths, enumerate_ns, sort_ns)
}

fn run_case(root: &Path, supported: usize) {
    let folder = build_folder(root, supported);
    let iterations = iterations_for(supported);

    // Quasi-cold sample: first scan after creation. OS page-cache state is
    // not controlled (no root to drop caches), so this is reported as an
    // upper-bound hint, not a true cold measurement.
    let cold_started = Instant::now();
    let cold_result = std::hint::black_box(scan_folder(&folder));
    let cold_ns = u64::try_from(cold_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let cold_rows = cold_result.len();

    for _ in 0..WARMUP_RUNS {
        std::hint::black_box(scan_folder(&folder));
    }

    let mut samples_ns = Vec::with_capacity(iterations);
    let mut enumerate_ns = Vec::with_capacity(iterations);
    let mut sort_ns = Vec::with_capacity(iterations);
    let mut result_len = 0;
    for _ in 0..iterations {
        let started = Instant::now();
        let production = scan_folder(&folder);
        samples_ns.push(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
        result_len = production.len();

        let (instrumented, e_ns, s_ns) = scan_folder_instrumented(&folder);
        assert_eq!(
            production, instrumented,
            "instrumented replica diverged from production scan_folder"
        );
        enumerate_ns.push(e_ns);
        sort_ns.push(s_ns);
    }

    samples_ns.sort_unstable();
    enumerate_ns.sort_unstable();
    sort_ns.sort_unstable();

    println!(
        "folder_scan supported={supported} result_rows={result_len} \
         quasi_cold_ms={:.3} quasi_cold_rows={cold_rows} \
         warm_p50_ms={:.3} warm_p95_ms={:.3} warm_max_ms={:.3} \
         enumerate_stat_p50_ms={:.3} enumerate_stat_p95_ms={:.3} \
         sort_p50_ms={:.3} sort_p95_ms={:.3} iterations={iterations}",
        ms(cold_ns),
        ms(percentile(&samples_ns, 50)),
        ms(percentile(&samples_ns, 95)),
        ms(*samples_ns.last().unwrap_or(&0)),
        ms(percentile(&enumerate_ns, 50)),
        ms(percentile(&enumerate_ns, 95)),
        ms(percentile(&sort_ns, 50)),
        ms(percentile(&sort_ns, 95)),
    );
}

fn main() {
    let root = std::env::temp_dir().join(format!("rrrah-folder-scan-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("clean stale synthetic folders");
    }
    fs::create_dir_all(&root).expect("create benchmark root");

    for &supported in &SUPPORTED_COUNTS {
        run_case(&root, supported);
    }

    fs::remove_dir_all(&root).expect("remove synthetic folders");
}
