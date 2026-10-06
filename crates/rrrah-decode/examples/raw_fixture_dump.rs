//! Dump native sensor samples and resolved metadata for independent qualification.
use std::{
    fs::File,
    io::{BufWriter, Write},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 && !(args.len() == 4 && args[3] == "--preview") {
        return Err("usage: raw_fixture_dump RAW OUTPUT_PREFIX [--preview]".into());
    }
    let budget = std::env::var("RRRAH_RAW_MEMORY_MB")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .map(|mb| rrrah_core::MemoryBudget::new(mb.saturating_mul(1024 * 1024)))
        })
        .transpose()?;
    let mut request = rrrah_decode::DecodeRequest::new(&args[1]);
    request.memory_budget = budget.clone();
    use rrrah_decode::RawDecoder;
    let decoded = rrrah_decode::NativeRawDecoder.decode(&request)?;
    if budget.is_some() && !decoded.mosaic.pixels.is_managed() {
        return Err("decoder output is not managed by the requested budget".into());
    }
    let metadata = &decoded.mosaic.metadata;
    let mut pixels = BufWriter::new(File::create(format!("{}.u16le", args[2]))?);
    for sample in decoded.mosaic.pixels.iter() {
        pixels.write_all(&sample.to_le_bytes())?;
    }
    pixels.flush()?;
    let cfa: Vec<_> = metadata
        .cfa
        .as_ref()
        .ok_or("missing CFA")?
        .cells
        .iter()
        .map(|c| match c {
            rrrah_core::CfaColor::Red => "R",
            rrrah_core::CfaColor::Green => "G",
            rrrah_core::CfaColor::Blue => "B",
            rrrah_core::CfaColor::Emerald => "E",
            _ => "?",
        })
        .collect();
    let crop = metadata.effective_crop();
    let camera_to_rgb: Vec<Vec<f64>> = if metadata.cfa.as_ref().is_some_and(|c| c.rgbe_quad().is_ok()) {
        rrrah_core::camera4_to_linear_srgb_precise(metadata.xyz_to_camera.map(|r| r.map(f64::from)))?
            .into_iter()
            .map(|r| r.to_vec())
            .collect()
    } else {
        rrrah_core::camera_to_linear_srgb(metadata.xyz_to_camera)
            .ok_or("invalid color profile")?
            .into_iter()
            .map(|r| r.into_iter().map(f64::from).collect())
            .collect()
    };
    let json = format!(
        "{{\"make\":{:?},\"model\":{:?},\"width\":{},\"height\":{},\"wb\":{:?},\"black_grid\":{:?},\"white\":{:?},\"cfa\":{:?},\"xyz_to_camera\":{:?},\"camera_to_rgb\":{:?},\"crop\":[{},{},{},{}]}}\n",
        metadata.make,
        metadata.model,
        metadata.width,
        metadata.height,
        metadata.white_balance,
        metadata.black_level.values,
        metadata.white_level.0,
        cfa,
        metadata.xyz_to_camera,
        camera_to_rgb,
        crop.x,
        crop.y,
        crop.width,
        crop.height
    );
    let json = if let Some(budget) = &budget {
        format!(
            "{},\"managed_memory\":{{\"limit\":{},\"used\":{},\"peak\":{},\"pixel_capacity\":{},\"managed\":true}}}}\n",
            json.trim_end().strip_suffix('}').unwrap(),
            budget.limit(),
            budget.used(),
            budget.peak(),
            decoded.mosaic.pixels.capacity_bytes()
        )
    } else {
        json
    };
    std::fs::write(format!("{}.json", args[2]), json)?;
    if args.len() == 4 {
        let (width, height) = metadata.thumbnail_dimensions(640);
        let rgba = decoded.mosaic.thumbnail_rgba8(640);
        if rgba.len() != usize::try_from(width)? * usize::try_from(height)? * 4 {
            return Err("color preview failed".into());
        }
        let mut preview = BufWriter::new(File::create(format!("{}.ppm", args[2]))?);
        write!(preview, "P6\n{width} {height}\n255\n")?;
        for pixel in rgba.chunks_exact(4) {
            preview.write_all(&pixel[..3])?;
        }
        preview.flush()?;
    }
    Ok(())
}
