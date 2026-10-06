//! Export quantized entropy coefficients for independent VC-5 oracle checks.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: vc5_entropy_dump BLOCK COEFFICIENTS OUTPUT".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let count: usize = args[2].parse()?;
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[3])?);
    for run in rrrah_decode::vc5::Vc5Runs::new(&bytes, count) {
        let (length, value) = run?;
        for _ in 0..length {
            output.write_all(&value.to_le_bytes())?;
        }
    }
    output.flush()?;
    Ok(())
}
