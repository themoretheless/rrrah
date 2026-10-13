//! Full-source HEIF native decode and PQ preparation, measured separately.
//! Warm filesystem; no GPU, persistent cache or physical HDR claim.
use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::{DecodeRequest, decode_raster};
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: heif_hdr_timing FILE ITERATIONS [table|scalar|compare]".into());
    }
    let count: usize = args[2].parse()?;
    if !(3..=20).contains(&count) {
        return Err("iterations must be 3..20".into());
    }
    let mode = args.get(3).map(String::as_str).unwrap_or("table");
    if !matches!(mode, "table" | "scalar" | "compare") {
        return Err("mode must be table, scalar or compare".into());
    }
    let budget = MemoryBudget::new(1024 * 1024 * 1024);
    let mut request = DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let mut reference = None;
    for iteration in 0..=count {
        let started = Instant::now();
        let native = decode_raster(black_box(&request))?;
        let native_ms = started.elapsed().as_secs_f64() * 1000.;
        let modes: &[&str] = match mode {
            "compare" if iteration % 2 == 0 => &["scalar", "table"],
            "compare" => &["table", "scalar"],
            "scalar" => &["scalar"],
            _ => &["table"],
        };
        let mut records = Vec::new();
        for &selected in modes {
            let output_bytes = u64::from(native.width()) * u64::from(native.height()) * 16;
            // Exact output credit forces the production scalar fallback; both
            // paths remain children of the same root and use the same source.
            let conversion = budget.child(output_bytes + if selected == "table" { 65536 * 8 } else { 0 });
            let started = Instant::now();
            let linear =
                native.pq_to_linear_srgb_with_budget_and_cancel(100., Some(&conversion), || false)?;
            let pq_ms = started.elapsed().as_secs_f64() * 1000.;
            assert_eq!(linear.color_space(), &RasterColorSpace::LinearSrgb);
            let RasterPixels::Rgba32Float(pixels) = linear.pixels() else {
                panic!()
            };
            let mut hash = blake3::Hasher::new();
            // Batch portable little-endian float bits outside the timed section.
            for values in pixels.chunks(4096) {
                let mut bytes = [0u8; 4096 * 4];
                for (v, dst) in values.iter().zip(bytes.chunks_exact_mut(4)) {
                    dst.copy_from_slice(&v.to_bits().to_le_bytes());
                }
                hash.update(&bytes[..values.len() * 4]);
            }
            let digest = hash.finalize().to_hex().to_string();
            if let Some(expected) = &reference {
                assert_eq!(&digest, expected);
            }
            reference = Some(digest.clone());
            black_box(&linear);
            records.push(serde_json::json!({"iteration": iteration, "mode": selected,
                "dimensions": [linear.width(), linear.height()], "native_ms": native_ms,
                "pq_ms": pq_ms, "float_blake3": digest, "conversion_peak": conversion.peak()}));
            drop(linear);
            assert_eq!(conversion.used(), 0);
        }
        drop(native);
        assert_eq!(budget.used(), 0);
        if iteration > 0 {
            for mut record in records {
                record["root"] = 0.into();
                record["peak"] = budget.peak().into();
                println!("{record}");
            }
        }
    }
    Ok(())
}
