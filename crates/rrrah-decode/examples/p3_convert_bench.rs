//! Paired current scalar/LUT P3 conversion, selected by optional scratch budget.
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use std::{sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count = 4000usize * 3000;
    let source = DecodedRaster::new(
        4000,
        3000,
        RasterPixels::Rgba16(
            Arc::new(
                (0..count)
                    .flat_map(|i| {
                        [
                            (i % 65536) as u16,
                            ((i * 37) % 65536) as u16,
                            ((i * 113) % 65536) as u16,
                            32768,
                        ]
                    })
                    .collect::<Vec<_>>(),
            )
            .into(),
        ),
        RasterColorSpace::Cicp {
            primaries: 12,
            transfer: 13,
            matrix: 0,
            full_range: true,
        },
    )?;
    let mut expected = None;
    let mut rows = Vec::new();
    for block in 0..12 {
        for table in [false, true, true, false] {
            let bytes = count as u64 * 16;
            let root = MemoryBudget::new(bytes + if table { 65536 * 4 } else { 0 });
            let start = Instant::now();
            let output = source.to_linear_srgb_with_budget(Some(&root))?;
            let ms = start.elapsed().as_secs_f64() * 1000.;
            let RasterPixels::Rgba32Float(values) = output.pixels() else {
                panic!()
            };
            let mut hash = blake3::Hasher::new();
            for value in values.iter() {
                hash.update(&value.to_bits().to_le_bytes());
            }
            let digest = hash.finalize().to_hex().to_string();
            if let Some(previous) = &expected {
                assert_eq!(previous, &digest);
            }
            expected = Some(digest);
            assert_eq!(root.peak(), root.limit());
            drop(output);
            assert_eq!(root.used(), 0);
            if block >= 2 {
                rows.push(
                    serde_json::json!({"block":block,"table":table,"ms":ms,"managed_peak":root.peak()}),
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"dimensions":[4000,3000],"warmup_blocks":2,
        "samples":rows,"output_blake3":expected,"managed_final_bytes":0,
        "scope":"Current scalar versus optional LUT P3 RGB16 CPU conversion; ABBA; source/hash/drop outside timer; no file IO or GPU"})
    );
    Ok(())
}
