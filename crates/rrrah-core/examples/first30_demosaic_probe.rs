//! Deterministic AMaZE output fingerprint and operation timing for RR-0026.
use rrrah_core::{CfaColor, CfaPattern, DemosaicAlgorithm, DemosaicConfig, MemoryBudget};
use std::{io::Write, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let large = args.iter().any(|a| a == "--large");
    let mut dump = args
        .windows(2)
        .find(|a| a[0] == "--dump")
        .map(|a| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&a[1])
        })
        .transpose()?;
    let sizes = if large {
        vec![(1024u32, 768u32)]
    } else {
        vec![
            (1, 1),
            (2, 3),
            (3, 2),
            (127, 129),
            (128, 128),
            (129, 127),
            (257, 255),
        ]
    };
    let cfa = CfaPattern {
        width: 2,
        height: 2,
        cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
    };
    let config = DemosaicConfig {
        algorithm: DemosaicAlgorithm::Amaze,
        ..Default::default()
    };
    print!("{{\"samples\":[");
    let mut comma = "";
    for (width, height) in sizes {
        let samples: Vec<u16> = (0..width * height)
            .map(|i| ((i.wrapping_mul(7919) ^ (i / width).wrapping_mul(3571)) % 16384) as u16)
            .collect();
        let budget = MemoryBudget::new(256 * 1024 * 1024);
        for iteration in 0..if large { 4 } else { 1 } {
            let start = Instant::now();
            let rgb = rrrah_core::develop::demosaic_u16_f64(
                &samples,
                width,
                height,
                0.,
                16383.,
                &cfa,
                &config,
                Some(&budget),
                &|| false,
            )?;
            let ns = start.elapsed().as_nanos();
            let mut hash = 14695981039346656037u64;
            for pixel in rgb.iter() {
                for value in pixel {
                    assert!(value.is_finite());
                    hash = (hash ^ value.to_bits()).wrapping_mul(1099511628211);
                }
            }
            if !large || iteration == 3 {
                if let Some(file) = dump.as_mut() {
                    file.write_all(&width.to_le_bytes())?;
                    file.write_all(&height.to_le_bytes())?;
                    for pixel in rgb.iter() {
                        for value in pixel {
                            file.write_all(&value.to_bits().to_le_bytes())?;
                        }
                    }
                }
            }
            drop(rgb);
            assert_eq!(budget.used(), 0);
            if !large || iteration == 3 {
                print!(
                    "{comma}{{\"width\":{width},\"height\":{height},\"ns\":{ns},\"f64_bits_hash\":{hash},\"managed_peak\":{}}}",
                    budget.peak()
                );
                comma = ",";
            }
        }
    }
    println!("]}}");
    Ok(())
}
