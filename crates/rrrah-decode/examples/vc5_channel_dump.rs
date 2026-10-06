//! Diagnostic baseline VC-5 component reconstruction, not a full GPR viewer.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: vc5_channel_dump ESSENCE CHANNEL OUTPUT.i16le".into());
    }
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let bytes = std::fs::read(&args[1])?;
    let index = rrrah_decode::vc5::index_raw(&bytes)?;
    let mut request = rrrah_decode::DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let channel = rrrah_decode::vc5::decode_channel(&index, args[2].parse()?, &request)?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[3])?);
    for value in channel.iter() {
        out.write_all(&value.to_le_bytes())?;
    }
    out.flush()?;
    println!(
        "samples={} peak_managed_bytes={} retained_bytes={}",
        channel.len(),
        budget.peak(),
        budget.used()
    );
    drop(channel);
    assert_eq!(budget.used(), 0);
    Ok(())
}
