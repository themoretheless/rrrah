//! Synthetic prefetch scheduler benchmark.
//!
//! The benchmark deliberately does not read or decode a RAW file. It holds
//! the worker in an injected loader while the producer repeatedly replaces
//! the pending viewport. This isolates the latency paid by the UI thread and
//! validates latest-generation publication plus the hard queue/output bound.

#![allow(clippy::cast_precision_loss)]

#[allow(dead_code, unused_imports)]
#[path = "../src/decode_gate.rs"]
mod decode_gate;

#[allow(
    clippy::bool_to_int_with_if,
    clippy::cast_possible_truncation,
    clippy::collapsible_if,
    clippy::manual_ok_err
)]
#[path = "../src/gallery.rs"]
mod gallery;

use crossbeam_channel::bounded;
use gallery::{Prefetcher, THUMB_EDGE, ThumbnailJob, ThumbnailReady};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const QUEUE_CAPACITY: usize = 8;
const BLOCKING_JOB: usize = usize::MAX;
const RGBA_BYTES_PER_THUMB: usize = THUMB_EDGE as usize * THUMB_EDGE as usize * 4;

fn job(index: usize) -> ThumbnailJob {
    ThumbnailJob {
        index,
        source: PathBuf::from("synthetic-no-io.dng"),
        edge: THUMB_EDGE,
    }
}

fn ready(index: usize) -> ThumbnailReady {
    ThumbnailReady {
        index,
        width: THUMB_EDGE,
        height: THUMB_EDGE,
        pixels: vec![0; RGBA_BYTES_PER_THUMB],
    }
}

fn percentile(sorted_ns: &[u64], percentile: usize) -> u64 {
    let numerator = sorted_ns.len().saturating_sub(1) * percentile;
    sorted_ns[numerator.div_ceil(100)]
}

fn benchmark_submit_latest_window(iterations: usize) {
    let (started_tx, started_rx) = bounded(1);
    let (release_tx, release_rx) = bounded(0);
    let prefetcher = Prefetcher::new(QUEUE_CAPACITY, move |thumbnail| {
        if thumbnail.index == BLOCKING_JOB {
            started_tx.send(()).expect("benchmark controller is alive");
            release_rx.recv().expect("benchmark controller releases worker");
        }
        Some(ready(thumbnail.index))
    });

    prefetcher.submit([job(BLOCKING_JOB)]);
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("synthetic worker did not start");

    let template = (0..QUEUE_CAPACITY).map(job).collect::<Vec<_>>();
    let mut samples_ns = Vec::with_capacity(iterations);
    let wall_started = Instant::now();
    for _ in 0..iterations {
        let started = Instant::now();
        prefetcher.submit(template.iter().cloned());
        samples_ns.push(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
    }
    let wall = wall_started.elapsed();
    samples_ns.sort_unstable();

    release_tx.send(()).expect("synthetic worker accepts release");

    println!(
        "prefetch_submit_latest_window iterations={iterations} window={QUEUE_CAPACITY} \
         p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3} submits_per_s={:.0}",
        percentile(&samples_ns, 50) as f64 / 1_000.0,
        percentile(&samples_ns, 95) as f64 / 1_000.0,
        percentile(&samples_ns, 99) as f64 / 1_000.0,
        samples_ns.last().copied().unwrap_or_default() as f64 / 1_000.0,
        iterations as f64 / wall.as_secs_f64(),
    );
}

fn validate_latest_wins_and_bounded_output() {
    let (started_tx, started_rx) = bounded(1);
    let (release_tx, release_rx) = bounded(0);
    let prefetcher = Prefetcher::new(QUEUE_CAPACITY, move |thumbnail| {
        if thumbnail.index == BLOCKING_JOB {
            started_tx.send(()).expect("validation controller is alive");
            release_rx.recv().expect("validation controller releases worker");
        }
        Some(ready(thumbnail.index))
    });

    prefetcher.submit([job(BLOCKING_JOB)]);
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("synthetic worker did not start");

    // This entire generation must be evicted while the worker is blocked.
    prefetcher.submit((0..QUEUE_CAPACITY).map(job));
    let newest_start = 10_000;
    prefetcher.submit((newest_start..newest_start + QUEUE_CAPACITY).map(job));
    release_tx.send(()).expect("synthetic worker accepts release");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut outputs = Vec::with_capacity(QUEUE_CAPACITY);
    while outputs.len() < QUEUE_CAPACITY && Instant::now() < deadline {
        if let Some(output) = prefetcher.try_recv() {
            outputs.push(output);
        } else {
            std::thread::yield_now();
        }
    }

    assert_eq!(outputs.len(), QUEUE_CAPACITY, "latest window was not completed");
    assert_eq!(
        outputs.iter().map(|output| output.index).collect::<Vec<_>>(),
        (newest_start..newest_start + QUEUE_CAPACITY).collect::<Vec<_>>(),
        "an obsolete generation reached publication",
    );
    let queued_output_bytes = outputs.iter().map(|output| output.pixels.len()).sum::<usize>();
    assert_eq!(queued_output_bytes, QUEUE_CAPACITY * RGBA_BYTES_PER_THUMB);
    assert!(prefetcher.try_recv().is_none(), "ready queue exceeded its bound");

    println!(
        "prefetch_correctness latest_wins=true queued_jobs_max={QUEUE_CAPACITY} \
         queued_ready_buffers_max={QUEUE_CAPACITY} queued_ready_bytes_max={queued_output_bytes} \
         scheduler_pixel_peak_bound={}",
        (QUEUE_CAPACITY + 1) * RGBA_BYTES_PER_THUMB,
    );
}

fn main() {
    let iterations = std::env::var("RRRAH_PREFETCH_BENCH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50_000);
    assert!(iterations > 0, "RRRAH_PREFETCH_BENCH_ITERS must be positive");

    benchmark_submit_latest_window(iterations);
    validate_latest_wins_and_bounded_output();
}
