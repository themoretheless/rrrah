//! Paired warm-source CPU timing; no preview, GUI or GPU claim.
use rrrah_decode::{DecodeRequest, NativeCr3Decoder, RawDecoder};
use std::{hint::black_box, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected CR3 path")?;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut request = DecodeRequest::new(path);
    request.memory_budget = Some(budget.clone());
    let mut digest = None;
    let mut rows = Vec::new();
    for round in 0..16 {
        let order = if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        };
        for prepared in order {
            let start = Instant::now();
            let decoded = if prepared {
                NativeCr3Decoder.prepare(black_box(&request))?.decode()?
            } else {
                NativeCr3Decoder.decode(black_box(&request))?
            };
            let ms = start.elapsed().as_secs_f64() * 1000.;
            let mut hash = blake3::Hasher::new();
            for v in decoded.mosaic.pixels.iter() {
                hash.update(&v.to_le_bytes());
            }
            hash.update(format!("{:?}", decoded.mosaic.metadata).as_bytes());
            let current = hash.finalize().to_hex().to_string();
            if let Some(expected) = &digest {
                assert_eq!(expected, &current);
            }
            digest = Some(current);
            drop(decoded);
            assert_eq!(budget.used(), 0);
            if round >= 4 {
                rows.push(serde_json::json!({"round":round,"prepared":prepared,"ms":ms}));
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"rows":rows,"digest":digest,"managed_peak":budget.peak(),"managed_final":budget.used()})
    );
    Ok(())
}
