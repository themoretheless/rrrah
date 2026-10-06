//! Public PDF load benchmark including filesystem reads and display preparation.
use rrrah_decode::{DecodeRequest, decode_raster, prepare_raster_for_display_with_budget};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args().nth(1).ok_or("usage: pdf_cmyk_load_bench FILE")?;
    let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
    let mut request = DecodeRequest::new(file);
    request.memory_budget = Some(budget.clone());
    let start = Instant::now();
    let cold = decode_raster(&request)?;
    let first_ms = start.elapsed().as_secs_f64() * 1000.0;
    let dimensions = [cold.width(), cold.height()];
    drop(cold);
    assert_eq!(budget.used(), 0);
    let mut decode_ms = Vec::new();
    let mut prepare_ms = Vec::new();
    let mut checksum = 0u64;
    for _ in 0..5 {
        let mut decode_total = 0.0;
        let mut prepare_total = 0.0;
        for _ in 0..32 {
            let start = Instant::now();
            let image = decode_raster(&request)?;
            decode_total += start.elapsed().as_secs_f64();
            let start = Instant::now();
            let display = prepare_raster_for_display_with_budget(&image, Some(&budget))?;
            prepare_total += start.elapsed().as_secs_f64();
            checksum += std::hint::black_box(display.capacity_bytes());
            drop((display, image));
            assert_eq!(budget.used(), 0);
        }
        decode_ms.push(decode_total * 1000.0 / 32.0);
        prepare_ms.push(prepare_total * 1000.0 / 32.0);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "dimensions": dimensions, "first_decode_ms": first_ms,
            "warm_decode_ms": decode_ms, "warm_prepare_ms": prepare_ms,
            "managed_peak_bytes": budget.peak(), "managed_final_bytes": budget.used(),
            "checksum": checksum, "iterations_per_repeat":32,"repeats":5,
            "scope":"Public decoder and display preparation, cached filesystem, no GPU rendering; first decode includes first-use ICC initialization, not disk-cold I/O"
        }))?
    );
    Ok(())
}
