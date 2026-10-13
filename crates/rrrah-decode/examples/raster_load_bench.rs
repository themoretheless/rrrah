//! Warm filesystem, in-process decode + display-preparation timings; no GPU claim.
use rrrah_core::RasterPixels;
use rrrah_decode::{DecodeRequest, decode_raster, prepare_raster_for_display_with_budget};
use std::{hint::black_box, time::Instant};
fn fingerprint(pixels: &RasterPixels) -> String {
    let mut hash = blake3::Hasher::new();
    match pixels {
        RasterPixels::Rgba8(v) => {
            hash.update(v);
        }
        RasterPixels::Rgba16(v) => {
            for x in v.iter() {
                hash.update(&x.to_le_bytes());
            }
        }
        RasterPixels::Rgba32Float(v) => {
            for x in v.iter() {
                hash.update(&x.to_le_bytes());
            }
        }
    }
    hash.finalize().to_hex().to_string()
}
fn percentile(values: &mut [f64], percent: usize) -> f64 {
    assert!(!values.is_empty() && (1..=100).contains(&percent));
    values.sort_by(f64::total_cmp);
    let rank = values.len().checked_mul(percent).unwrap().div_ceil(100);
    values[rank - 1]
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 6 && args[4] == "--cancel-after-ms" {
        return cancellation_bench(&args);
    }
    let managed_srgb = args.len() == 5 && args[4] == "--managed-srgb";
    if args.len() != 4 && !managed_srgb {
        return Err("usage: raster_load_bench FILE INDEX ITERATIONS [--managed-srgb]".into());
    }
    let mut request = DecodeRequest::new(&args[1]);
    request.image_index = args[2].parse()?;
    if managed_srgb {
        request.assume_untagged_srgb = true;
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(512 * 1024 * 1024));
    }
    let count: usize = args[3].parse()?;
    if count < 3 {
        return Err("at least 3 iterations required".into());
    }
    for _ in 0..3 {
        black_box(prepare_raster_for_display_with_budget(&decode_raster(&request)?, request.memory_budget.as_ref())?);
    }
    let mut load = Vec::new();
    let mut prepare = Vec::new();
    let mut ready = Vec::new();
    let mut digest = None;
    for _ in 0..count {
        let started = Instant::now();
        let decoded = decode_raster(black_box(&request))?;
        let loaded = Instant::now();
        let display = prepare_raster_for_display_with_budget(black_box(&decoded), request.memory_budget.as_ref())?;
        let finished = Instant::now();
        load.push((loaded - started).as_secs_f64() * 1000.);
        prepare.push((finished - loaded).as_secs_f64() * 1000.);
        ready.push((finished - started).as_secs_f64() * 1000.);
        let current = fingerprint(display.pixels());
        if let Some(previous) = &digest {
            assert_eq!(previous, &current);
        }
        digest = Some(current);
        black_box(display);
    }
    if let Some(budget) = &request.memory_budget {
        assert_eq!(budget.used(), 0, "benchmark retained managed pixels");
        eprintln!("managed_srgb: limit_bytes={} peak_bytes={} final_bytes=0", budget.limit(), budget.peak());
    }
    println!(
        "{{\"path\":{:?},\"index\":{},\"iterations\":{},\"load_p50_ms\":{},\"load_p95_ms\":{},\"prepare_p50_ms\":{},\"prepare_p95_ms\":{},\"cpu_ready_p50_ms\":{},\"cpu_ready_p95_ms\":{},\"display_blake3\":{:?},\"cache\":\"warm filesystem; process warm; no decoded image cache\"}}",
        args[1],
        request.image_index,
        count,
        percentile(&mut load, 50),
        percentile(&mut load, 95),
        percentile(&mut prepare, 50),
        percentile(&mut prepare, 95),
        percentile(&mut ready, 50),
        percentile(&mut ready, 95),
        digest.unwrap()
    );
    Ok(())
}

fn cancellation_bench(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use rrrah_decode::{DecodeError, GenerationToken, RasterDecodeError};
    use std::sync::{Arc, Barrier, atomic::{AtomicU64, Ordering}};
    use std::time::Duration;
    let count: usize = args[3].parse()?;
    let delay: u64 = args[5].parse()?;
    let budget_mb = std::env::var("RRRAH_BENCH_MEMORY_MB").ok()
        .map(|value| value.parse::<u64>()).transpose()?.unwrap_or(64);
    if !(1..=4096).contains(&budget_mb) { return Err("benchmark budget must be 1..=4096 MiB".into()); }
    if count < 3 || delay > 1000 { return Err("at least 3 samples; cancellation delay at most 1000 ms".into()); }
    let mut rows = Vec::new();
    let mut latencies = Vec::new();
    let mut completed = 0;
    for _ in 0..count {
        let generation = Arc::new(AtomicU64::new(0));
        let budget = rrrah_core::MemoryBudget::new(budget_mb * 1024 * 1024);
        let mut request = DecodeRequest::new(&args[1]);
        request.image_index = args[2].parse()?;
        request.memory_budget = Some(budget.clone());
        request.cancellation = Some(GenerationToken::new(generation.clone(), 0));
        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = barrier.clone();
        let timer = std::thread::spawn(move || {
            worker_barrier.wait();
            std::thread::sleep(Duration::from_millis(delay));
            let cancelled_at = Instant::now();
            generation.store(1, Ordering::Release);
            cancelled_at
        });
        let started = Instant::now();
        barrier.wait();
        let result = decode_raster(&request);
        let finished = Instant::now();
        let cancelled_at = timer.join().map_err(|_| "cancellation timer panicked")?;
        let status = match result {
            Err(RasterDecodeError::Source(DecodeError::Cancelled)) => {
                let latency = finished.saturating_duration_since(cancelled_at).as_secs_f64() * 1000.;
                latencies.push(latency);
                "cancelled"
            }
            Ok(image) => { drop(image); completed += 1; "completed" }
            Err(error) => return Err(error.into()),
        };
        assert_eq!(budget.used(), 0, "cancelled/completed decode retained managed credit");
        let cancel_peak = budget.peak();
        let retry = if status == "cancelled" {
            request.cancellation = None;
            let image = decode_raster(&request)?;
            let dimensions = [image.width(), image.height()];
            drop(image);
            assert_eq!(budget.used(), 0, "retry retained managed pixels");
            Some(dimensions)
        } else { None };
        rows.push(serde_json::json!({
            "status": status,
            "decode_ms": finished.duration_since(started).as_secs_f64() * 1000.,
            "actual_signal_ms": cancelled_at.duration_since(started).as_secs_f64() * 1000.,
            "managed_peak_bytes": cancel_peak, "managed_final_bytes": budget.used(),
            "retry_dimensions": retry
        }));
    }
    let median = if latencies.is_empty() { None } else { Some(percentile(&mut latencies, 50)) };
    let tail = if latencies.is_empty() { None } else { Some(percentile(&mut latencies, 95)) };
    let maximum = latencies.iter().copied().reduce(f64::max);
    println!("{}", serde_json::to_string(&serde_json::json!({
        "path": args[1], "index": args[2], "samples": count, "requested_signal_ms": delay,
        "managed_budget_bytes": budget_mb * 1024 * 1024,
        "cancelled": latencies.len(), "completed": completed,
        "signal_to_decoder_exit_p50_ms": median, "signal_to_decoder_exit_p95_ms": tail,
        "signal_to_decoder_exit_max_ms": maximum, "rows": rows,
        "scope": "Warm OS filesystem, CPU decode, separate timer thread; includes scheduling and cleanup; no GPU presentation or universal latency bound"
    }))?);
    Ok(())
}

#[test]
fn nearest_rank_tail_for_short_runs_and_single_sample() {
    let mut short = [1., 2., 100.];
    assert_eq!(percentile(&mut short, 95), 100.);
    assert_eq!(percentile(&mut short, 50), 2.);
    let mut sixteen: Vec<_> = (1..=16).rev().map(f64::from).collect();
    assert_eq!(percentile(&mut sixteen, 95), 16.);
    assert_eq!(percentile(&mut sixteen, 50), 8.);
    assert_eq!(percentile(&mut [7.], 1), 7.);
    assert_eq!(percentile(&mut [7.], 100), 7.);
}
