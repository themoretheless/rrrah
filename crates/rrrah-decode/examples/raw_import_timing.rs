//! Warm-source native import timing; excludes rendering and presentation.
use rrrah_decode::{DecodeRequest, NativeRawDecoder, RawDecoder};
use std::{hint::black_box, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: raw_import_timing RAW REPS".into());
    }
    let reps: usize = args[2].parse()?;
    if !(1..=1000).contains(&reps) {
        return Err("REPS must be 1..=1000".into());
    }
    let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut request = DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let reference = NativeRawDecoder.decode(&request)?.mosaic;
    let mut hasher = blake3::Hasher::new();
    for pixel in reference.pixels.iter() {
        hasher.update(&pixel.to_le_bytes());
    }
    let digest = hasher.finalize();
    let expected_metadata = reference.metadata.clone();
    drop(reference);
    assert_eq!(budget.used(), 0);
    let mut wall = Vec::with_capacity(reps);
    let mut source = Vec::with_capacity(reps);
    let mut decode = Vec::with_capacity(reps);
    for _ in 0..reps {
        let started = Instant::now();
        let output = NativeRawDecoder.decode(&request)?;
        wall.push(started.elapsed().as_secs_f64() * 1000.);
        source.push(output.timings.source_open.as_secs_f64() * 1000.);
        decode.push(output.timings.raw_decode.as_secs_f64() * 1000.);
        let mut actual = blake3::Hasher::new();
        for pixel in output.mosaic.pixels.iter() {
            actual.update(&pixel.to_le_bytes());
        }
        assert_eq!(actual.finalize(), digest);
        assert_eq!(output.mosaic.metadata, expected_metadata);
        black_box(&output);
        drop(output);
        assert_eq!(budget.used(), 0);
    }
    fn quantiles(mut values: Vec<f64>) -> (f64, f64) {
        values.sort_by(f64::total_cmp);
        (
            values[(values.len() - 1) / 2],
            values[(values.len() * 95).div_ceil(100) - 1],
        )
    }
    let (wall50, wall95) = quantiles(wall);
    let (source50, source95) = quantiles(source);
    let (decode50, decode95) = quantiles(decode);
    println!(
        "{{\"reps\":{reps},\"warmups\":1,\"wall_ms\":{{\"p50\":{wall50},\"p95\":{wall95}}},\"source_ms\":{{\"p50\":{source50},\"p95\":{source95}}},\"decode_ms\":{{\"p50\":{decode50},\"p95\":{decode95}}},\"managed_peak_bytes\":{},\"pixel_blake3\":\"{}\"}}",
        budget.peak(),
        digest.to_hex()
    );
    Ok(())
}
