//! Selected RGBA16 controls against C oracles; also checks exact managed peak/refusal.
use rrrah_core::{MemoryBudget, RasterPixels};
use rrrah_decode::{DecodeRequest, decode_raster};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: jxl_selected_probe FILE INDEX EXPECTED_COUNT".into());
    }
    let index: usize = args[2].parse()?;
    let count: usize = args[3].parse()?;
    let root = MemoryBudget::new(512 * 1024 * 1024);
    let mut request = DecodeRequest::new(&args[1]);
    request.image_index = index;
    request.memory_budget = Some(root.clone());
    let result = decode_raster(&request);
    let mut row = serde_json::json!({"index":index,"expected_count":count,"passed":false});
    match result {
        Ok(frame) => {
            let RasterPixels::Rgba16(values) = frame.pixels() else {
                return Err("expected RGBA16".into());
            };
            let raw = std::fs::read(format!("{}.{index}.rgba16le", args[1]))?;
            let expected: Vec<u16> = raw
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let exact = values.as_ref() == expected;
            let selection = (frame.image_index(), frame.image_count()) == (index, count);
            let timing = frame.animation_timing().ok_or("missing timing")?;
            let expected_delay = [(1001, 8), (7007, 24), (11011, 24)][index % 3];
            let timing_exact = (timing.delay_ms_numerator(), timing.delay_ms_denominator()) == expected_delay
                && timing.num_plays() == 2;
            row["exact_native"] = exact.into();
            row["selection_exact"] = selection.into();
            row["timing_exact"] = timing_exact.into();
            row["passed"] = (exact && selection && timing_exact).into();
            drop(frame);
            assert_eq!(root.used(), 0);
            let peak = root.peak();
            for limit in [peak - 1, peak] {
                let bounded = MemoryBudget::new(limit);
                request.memory_budget = Some(bounded.clone());
                let result = decode_raster(&request);
                if limit < peak {
                    assert!(matches!(
                        result,
                        Err(rrrah_decode::RasterDecodeError::Source(
                            rrrah_decode::DecodeError::Memory(_)
                        ))
                    ));
                } else {
                    assert!(result.is_ok());
                }
                drop(result);
                assert_eq!(bounded.used(), 0);
            }
            row["exact_peak_retry_and_one_byte_short_refusal"] = true.into();
        }
        Err(error) => row["error"] = format!("{error:?}").into(),
    }
    assert_eq!(root.used(), 0);
    row["root_peak"] = root.peak().into();
    row["root_final"] = root.used().into();
    println!("{row}");
    if row["passed"] != true {
        return Err("selected source qualification failed".into());
    }
    Ok(())
}
