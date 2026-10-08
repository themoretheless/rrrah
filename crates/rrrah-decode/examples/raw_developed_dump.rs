//! Dump the production scene-linear RAW development path for independent comparison.
use rrrah_decode::RawDecoder;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(args.len() == 3 || (args.len() == 4 && args[3] == "--no-highlight-recovery")) {
        return Err("usage: raw_developed_dump INPUT OUTPUT.rgba32fle [--no-highlight-recovery]".into());
    }
    let budget = rrrah_core::MemoryBudget::new(2 * 1024 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(&args[1]);
    request.memory_budget = Some(budget.clone());
    let mosaic = rrrah_decode::NativeRawDecoder.decode(&request)?.mosaic;
    let lists = rrrah_decode::raw_development_opcodes(&request)?;
    eprintln!(
        "camera={} {} white_balance={:?} xyz_to_camera={:?}",
        mosaic.metadata.make,
        mosaic.metadata.model,
        mosaic.metadata.white_balance,
        mosaic.metadata.xyz_to_camera
    );
    let options = rrrah_core::develop::DevelopOptions {
        recover_highlights: args.len() == 3,
        ..Default::default()
    };
    eprintln!("highlight_recovery={}", options.recover_highlights);
    let raster =
        rrrah_core::develop::develop_raw(&mosaic, &options, &lists, Some(&budget), &|| false)?;
    let rrrah_core::RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
        return Err("expected linear float development".into());
    };
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[2])?);
    for value in pixels.iter() {
        output.write_all(&value.to_le_bytes())?;
    }
    output.flush()?;
    eprintln!(
        "dimensions={}x{} color={:?} peak={}",
        raster.width(),
        raster.height(),
        raster.color_space(),
        budget.peak()
    );
    drop(raster);
    drop(mosaic);
    assert_eq!(budget.used(), 0);
    Ok(())
}
