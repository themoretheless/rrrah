//! Reproducible end-to-end CR3 timing harness used for cross-worktree A/B.

use std::{
    env,
    error::Error,
    hint::black_box,
    path::Path,
    time::{Duration, Instant},
};

use rrrah_decode::decode_file;

const REPS_ENV: &str = "RRRAH_CR3_BENCH_REPS";
const WARMUPS_ENV: &str = "RRRAH_CR3_BENCH_WARMUPS";
const VARIANT_ENV: &str = "RRRAH_CR3_BENCH_VARIANT";

fn env_count(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    let value = env::var(name).map_or(Ok(default), |raw| raw.parse::<usize>())?;
    if value == 0 {
        return Err(format!("{name} must be positive").into());
    }
    Ok(value)
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

// Percentile index math is inherently float-based; sample counts are tiny, so the casts are exact.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    let index = ((values.len() - 1) as f64 * fraction).round() as usize;
    values[index.min(values.len() - 1)]
}

fn pixel_digest(pixels: &[u16]) -> String {
    let mut hasher = blake3::Hasher::new();
    for pixel in pixels {
        hasher.update(&pixel.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn run_fixture(variant: &str, path: &Path, warmups: usize, reps: usize) -> Result<(), Box<dyn Error>> {
    for _ in 0..warmups {
        black_box(decode_file(path)?);
    }

    let fixture = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<non-utf8>");
    let mut expected_digest = None;
    let mut wall_values = Vec::with_capacity(reps);
    let mut total_values = Vec::with_capacity(reps);
    let mut plane_values = Vec::with_capacity(reps);
    let mut interleave_values = Vec::with_capacity(reps);
    let mut expected_workers = None;

    for iteration in 1..=reps {
        let started = Instant::now();
        let output = black_box(decode_file(path)?);
        let wall = started.elapsed();
        let native = output
            .timings
            .native
            .ok_or("fixture did not use the native CR3 decoder")?;
        let digest = pixel_digest(&output.mosaic.pixels);
        if let Some(expected) = &expected_digest {
            if expected != &digest {
                return Err(format!("{fixture}: pixel digest changed between repetitions").into());
            }
        } else {
            expected_digest = Some(digest.clone());
        }
        if let Some(workers) = expected_workers {
            if workers != native.worker_count {
                return Err(format!("{fixture}: worker count changed between repetitions").into());
            }
        } else {
            expected_workers = Some(native.worker_count);
        }
        wall_values.push(milliseconds(wall));
        total_values.push(milliseconds(output.timings.total));
        plane_values.push(milliseconds(native.plane_wall));
        interleave_values.push(milliseconds(native.interleave));
        println!(
            "sample,{variant},{fixture},{iteration},{},{:.3},,{:.3},,{:.3},,{:.3},,{digest}",
            native.worker_count,
            milliseconds(wall),
            milliseconds(output.timings.total),
            milliseconds(native.plane_wall),
            milliseconds(native.interleave),
        );
    }

    println!(
        "summary,{variant},{fixture},{reps},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{}",
        expected_workers.expect("at least one repetition"),
        percentile(&mut wall_values, 0.50),
        percentile(&mut wall_values, 0.95),
        percentile(&mut total_values, 0.50),
        percentile(&mut total_values, 0.95),
        percentile(&mut plane_values, 0.50),
        percentile(&mut plane_values, 0.95),
        percentile(&mut interleave_values, 0.50),
        percentile(&mut interleave_values, 0.95),
        expected_digest.expect("at least one repetition"),
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("pass one or more CR3 fixture paths".into());
    }
    let reps = env_count(REPS_ENV, 9)?;
    let warmups = env_count(WARMUPS_ENV, 2)?;
    let variant = env::var(VARIANT_ENV).unwrap_or_else(|_| "unknown".into());
    println!(
        "kind,variant,fixture,iteration_or_reps,workers,wall_ms,wall_p95_ms,total_ms,total_p95_ms,plane_wall_ms,plane_p95_ms,interleave_ms,interleave_p95_ms,digest"
    );
    for path in paths {
        run_fixture(&variant, Path::new(&path), warmups, reps)?;
    }
    Ok(())
}
