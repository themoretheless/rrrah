//! Export selected native RGBA8 for independent pixel-oracle comparisons.
use rrrah_decode::{DecodeRequest, decode_raster};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let linear = args.len() == 5 && args[4] == "--linear";
    if args.len() != 4 && !linear {
        return Err("usage: raster_fixture_dump FILE INDEX OUTPUT [--linear]".into());
    }
    let mut request = DecodeRequest::new(&args[1]);
    request.image_index = args[2].parse()?;
    request.memory_budget = Some(rrrah_core::MemoryBudget::new(512 * 1024 * 1024));
    let image = decode_raster(&request)?;
    if linear {
        use std::io::Write;
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&image, request.memory_budget.as_ref())?;
        let rrrah_core::RasterPixels::Rgba32Float(values) = prepared.pixels() else {
            return Err("prepared sample representation is not RGBA32F".into());
        };
        let mut output = std::io::BufWriter::new(std::fs::File::create(&args[3])?);
        for value in values.iter() {
            output.write_all(&value.to_le_bytes())?;
        }
        output.flush()?;
        println!("linear_rgba32f {}x{}", prepared.width(), prepared.height());
        drop(prepared);
        drop(image);
        if request
            .memory_budget
            .as_ref()
            .is_some_and(|budget| budget.used() != 0)
        {
            return Err("managed raster buffers not released".into());
        }
        return Ok(());
    }
    let rrrah_core::RasterPixels::Rgba8(values) = image.pixels() else {
        return Err("native sample representation is not RGBA8".into());
    };
    std::fs::write(&args[3], &**values)?;
    println!(
        "{{\"width\":{},\"height\":{},\"image_index\":{},\"image_count\":{}}}",
        image.width(),
        image.height(),
        image.image_index(),
        image.image_count()
    );
    Ok(())
}
