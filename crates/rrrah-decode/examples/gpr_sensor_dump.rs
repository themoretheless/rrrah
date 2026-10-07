//! Diagnostic ordinary native DNG/GPR admission with source and sensor accounting.
use rrrah_decode::RawDecoder;
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: gpr_sensor_dump INPUT.GPR OUTPUT.u16le".into());
    }
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let decoded = rrrah_decode::NativeDngDecoder.decode(&request)?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[2])?);
    for value in decoded.mosaic.pixels.iter() {
        out.write_all(&value.to_le_bytes())?;
    }
    out.flush()?;
    println!(
        "samples={} peak={} retained={}",
        decoded.mosaic.pixels.len(),
        budget.peak(),
        budget.used()
    );
    drop(decoded);
    assert_eq!(budget.used(), 0);
    Ok(())
}
