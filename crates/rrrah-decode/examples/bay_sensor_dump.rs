//! Explicit-layout sensor-only dump for external qualification; no viewer routing.
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: bay_sensor_dump qv2000ux|qv3000ex|qv5700 INPUT OUTPUT".into());
    }
    use rrrah_decode::BaySensorLayout;
    let layout = match args[1].as_str() {
        "qv2000ux" => BaySensorLayout::Qv2000Ux,
        "qv3000ex" => BaySensorLayout::Qv3000Ex,
        "qv5700" => BaySensorLayout::Qv5700,
        _ => return Err("unknown explicit camera layout".into()),
    };
    let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
    let samples = rrrah_decode::read_bay_sensor_with_budget(
        &rrrah_decode::DecodeRequest::new(&args[2]),
        layout,
        &budget,
    )?;
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[3])?);
    for sample in samples.iter() {
        output.write_all(&sample.to_le_bytes())?;
    }
    output.flush()?;
    drop(samples);
    if budget.used() != 0 {
        return Err("sensor lease was not released".into());
    }
    println!("dimensions={:?} managed_final=0", layout.dimensions());
    Ok(())
}
