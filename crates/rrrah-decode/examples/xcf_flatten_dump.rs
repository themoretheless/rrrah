//! Encoded RGBA diagnostics for the supported legacy XCF subset.
use rrrah_core::{MemoryBudget, RasterPixels};
use rrrah_decode::{DecodeRequest, decode_raster};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: xcf_flatten_dump FILE RGBA_OUTPUT")?;
    let output = args.next().ok_or("usage: xcf_flatten_dump FILE RGBA_OUTPUT")?;
    if args.next().is_some() {
        return Err("usage: xcf_flatten_dump FILE RGBA_OUTPUT".into());
    }
    let budget = MemoryBudget::new(512 * 1024 * 1024);
    let mut request = DecodeRequest::new(source);
    request.memory_budget = Some(budget.clone());
    let raster = decode_raster(&request)?;
    let RasterPixels::Rgba8(pixels) = raster.pixels() else {
        return Err("expected native RGBA8 output".into());
    };
    std::fs::write(output, &**pixels)?;
    println!(
        "{{\"width\":{},\"height\":{},\"rgba_blake3\":\"{}\",\"profile_bytes\":{}}}",
        raster.width(),
        raster.height(),
        blake3::hash(pixels),
        raster.color_profile_capacity_bytes()
    );
    drop(raster);
    if budget.used() != 0 {
        return Err("managed ownership retained".into());
    }
    println!("{{\"managed_peak\":{},\"managed_used\":0}}", budget.peak());
    Ok(())
}
