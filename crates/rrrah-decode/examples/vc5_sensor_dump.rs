//! Diagnostic baseline VC-5 Bayer reconstruction; DNG admission is separate.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: vc5_sensor_dump ESSENCE OUTPUT.u16le".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let index = rrrah_decode::vc5::index_raw(&bytes)?;
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let sensor = rrrah_decode::vc5::decode_bayer(&index, 14, rrrah_decode::vc5::BayerOrder::Rggb, &request)?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[2])?);
    for value in sensor.iter() {
        out.write_all(&value.to_le_bytes())?;
    }
    out.flush()?;
    println!(
        "samples={} peak_managed_bytes={} retained_bytes={}",
        sensor.len(),
        budget.peak(),
        budget.used()
    );
    drop(sensor);
    assert_eq!(budget.used(), 0);
    Ok(())
}
