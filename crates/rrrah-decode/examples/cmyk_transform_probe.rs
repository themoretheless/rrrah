//! Diagnostic only: isolates moxcms from PDF parsing and rasterization.
use moxcms::{BarycentricWeightScale, ColorProfile, InterpolationMethod, Layout, TransformOptions};
use std::{error::Error, fs};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: cmyk_transform_probe PROFILE ORACLE_JSON OUTPUT_PREFIX".into());
    }
    let profile = ColorProfile::new_from_slice(&fs::read(&args[1])?)?;
    let oracle: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let rows = oracle["normalized_f32_cmyk"]
        .as_array()
        .ok_or("missing operands")?;
    let input: Vec<f32> = rows
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    assert_eq!(input.len(), 48);
    let destination = ColorProfile::new_srgb();
    let float = profile.create_transform_f32(
        Layout::Rgba,
        &destination,
        Layout::Rgb,
        TransformOptions::default(),
    )?;
    let mut float_out = vec![0.0; 36];
    float.transform(&input, &mut float_out)?;
    let byte = profile.create_transform_8bit(
        Layout::Rgba,
        &destination,
        Layout::Rgb,
        TransformOptions::default(),
    )?;
    let quantized: Vec<u8> = input.iter().map(|v| (v * 255.0 + 0.5) as u8).collect();
    let mut byte_out = vec![0; 36];
    byte.transform(&quantized, &mut byte_out)?;
    let rounded: Vec<u8> = float_out.iter().map(|v| (v * 255.0 + 0.5) as u8).collect();
    for (mode, rgb) in [("float", &rounded), ("byte", &byte_out)] {
        let rgba: Vec<u8> = (0..8)
            .flat_map(|_| rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]))
            .collect();
        fs::write(format!("{}-{mode}.rgba", args[3]), rgba)?;
    }
    let mut variants = Vec::new();
    for (method_name, method) in [
        ("linear", InterpolationMethod::Linear),
        ("tetrahedral", InterpolationMethod::Tetrahedral),
        ("pyramid", InterpolationMethod::Pyramid),
        ("prism", InterpolationMethod::Prism),
    ] {
        for fixed in [false, true] {
            for (weight_name, weight) in [
                ("low", BarycentricWeightScale::Low),
                ("high", BarycentricWeightScale::High),
            ] {
                let options = TransformOptions {
                    prefer_fixed_point: fixed,
                    interpolation_method: method,
                    barycentric_weight_scale: weight,
                    ..TransformOptions::default()
                };
                let executor =
                    profile.create_transform_f32(Layout::Rgba, &destination, Layout::Rgb, options)?;
                let mut values = vec![0.0; 36];
                executor.transform(&input, &mut values)?;
                let rgb: Vec<u8> = values.iter().map(|v| (v * 255.0 + 0.5) as u8).collect();
                let mode = format!("{method_name}-fixed{fixed}-{weight_name}");
                let rgba: Vec<u8> = (0..8)
                    .flat_map(|_| rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]))
                    .collect();
                fs::write(format!("{}-{mode}.rgba", args[3]), rgba)?;
                let lab_profile = ColorProfile::new_lab();
                let lab = match profile.create_transform_f32(Layout::Rgba, &lab_profile, Layout::Rgb, options)
                {
                    Ok(executor) => {
                        let mut values = vec![0.0; 36];
                        executor.transform(&input, &mut values)?;
                        serde_json::json!({"values": values})
                    }
                    Err(error) => serde_json::json!({"error": format!("{error:?}")}),
                };
                variants
                    .push(serde_json::json!({"mode": mode, "float_rgb": values, "rgb8": rgb, "lab": lab}));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "variants": variants, "input": input, "float_rgb": float_out, "float_rgb8": rounded,
            "quantized_cmyk8": quantized, "byte_rgb8": byte_out,
            "scope": "Direct moxcms 0.8.1, default options, same profile, no PDF parser or renderer"
        }))?
    );
    Ok(())
}
