//! Qualification probe for tiny authored 8-bit animation controls, not viewer decoding.
use std::{io::Cursor, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("fixture path required")?);
    let bytes = std::fs::read(&path)?;
    let image = jxl_oxide::JxlImage::builder()
        .alloc_tracker(jxl_oxide::AllocTracker::with_limit(16 * 1024 * 1024))
        .read(Cursor::new(&bytes))?;
    if image.width() > 64 || image.height() > 64 {
        return Err("probe only accepts tiny authored controls".into());
    }
    if std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("--icc")) {
        let destination = std::env::args_os().nth(3).ok_or("ICC output path required")?;
        std::fs::write(destination, image.rendered_icc())?;
        return Ok(());
    }
    let animation = image
        .image_header()
        .metadata
        .animation
        .as_ref()
        .ok_or("not animated")?;
    if !image.is_loading_done() || image.num_loaded_keyframes() != 3 {
        return Err("expected three complete keyframes".into());
    }
    let mut frames = Vec::new();
    for index in 0..image.num_loaded_keyframes() {
        let render = image.render_frame(index)?;
        if u64::from(render.duration()) * u64::from(animation.tps_denominator) * 1000
            != [40u64, 90, 130][index] * u64::from(animation.tps_numerator)
        {
            return Err("duration differs from independent oracle".into());
        }
        let mut stream = render.stream();
        let channels = stream.channels() as usize;
        if !matches!(channels, 3 | 4) {
            return Err("probe expects RGB/RGBA8 controls".into());
        }
        let count = stream.width() as usize * stream.height() as usize;
        let mut samples = vec![0u8; count * channels];
        let written = stream.write_to_buffer(&mut samples);
        if written != samples.len() {
            return Err("incomplete sample stream".into());
        }
        let mut rgba = Vec::with_capacity(count * 4);
        for pixel in samples.chunks_exact(channels) {
            let alpha = if channels == 4 { pixel[3] } else { 255 };
            rgba.extend_from_slice(if alpha == 0 { &[0, 0, 0] } else { &pixel[..3] });
            rgba.push(alpha);
        }
        let golden = std::fs::read(format!("{}.{}.rgba", path.display(), index))?;
        if rgba != golden {
            return Err(format!("frame {index} differs from independent oracle").into());
        }
        frames.push(serde_json::json!({"index":index,"ticks":render.duration(),"rgba_blake3":blake3::hash(&rgba).to_hex().to_string(),"independent_rgba_exact":true}));
    }
    println!(
        "{}",
        serde_json::json!({"source_blake3":blake3::hash(&bytes).to_hex().to_string(),"width":image.width(),"height":image.height(),"ticks_per_second_numerator":animation.tps_numerator,"ticks_per_second_denominator":animation.tps_denominator,"num_loops":animation.num_loops,"frames":frames,"scope":"tiny controls; tracker limit16MiB is not a whole-process admission or viewer qualification"})
    );
    Ok(())
}
