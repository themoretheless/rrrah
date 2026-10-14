// Authored numeric fixture inputs are CC0-1.0. Independent encoder: ravif 0.13.0.
use ravif::{AlphaColorMode, BitDepth, ColorModel, Encoder, Img, RGBA8};
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&root).unwrap();
    let pixels: Vec<RGBA8> = (0..16).flat_map(|y| (0..16).map(move |x| {
        let alpha = [0u8, 1, 3, 10, 17, 64, 128, 255][x % 8];
        RGBA8::new((x * 17) as u8, (y * 17) as u8, ((x + y) * 8) as u8, alpha)
    })).collect();
    let rgba: Vec<u8> = pixels.iter().flat_map(|p| [p.r,p.g,p.b,p.a]).collect();
    std::fs::write(root.join("input.rgba"), rgba).unwrap();
    for (depth, label) in [(BitDepth::Eight,"8"),(BitDepth::Ten,"10")] {
        for (mode, name) in [(AlphaColorMode::UnassociatedDirty,"straight"),(AlphaColorMode::Premultiplied,"prem")] {
            let encoder = Encoder::new().with_quality(100.0).with_alpha_quality(100.0)
                .with_speed(10).with_num_threads(Some(1)).with_bit_depth(depth)
                .with_internal_color_model(ColorModel::RGB).with_alpha_color_mode(mode);
            let encoded = encoder.encode_rgba(Img::new(pixels.as_slice(),16,16)).unwrap();
            let path = root.join(format!("ravif-{label}bit-{name}.avif"));
            std::fs::write(&path, &encoded.avif_file).unwrap();
            println!("{}: file={}, color={}, alpha={}",path.display(),encoded.avif_file.len(),encoded.color_byte_size,encoded.alpha_byte_size);
        }
    }
}
