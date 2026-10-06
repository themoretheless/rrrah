//! Warm filesystem, in-process decode + display-preparation timings; no GPU claim.
use rrrah_core::RasterPixels;
use rrrah_decode::{DecodeRequest, decode_raster, prepare_raster_for_display};
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
    if args.len() != 4 {
        return Err("usage: raster_load_bench FILE INDEX ITERATIONS".into());
    }
    let mut request = DecodeRequest::new(&args[1]);
    request.image_index = args[2].parse()?;
    let count: usize = args[3].parse()?;
    if count < 3 {
        return Err("at least 3 iterations required".into());
    }
    for _ in 0..3 {
        black_box(prepare_raster_for_display(&decode_raster(&request)?)?);
    }
    let mut load = Vec::new();
    let mut prepare = Vec::new();
    let mut ready = Vec::new();
    let mut digest = None;
    for _ in 0..count {
        let started = Instant::now();
        let decoded = decode_raster(black_box(&request))?;
        let loaded = Instant::now();
        let display = prepare_raster_for_display(black_box(&decoded))?;
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
