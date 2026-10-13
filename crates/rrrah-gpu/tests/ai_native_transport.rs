//! Test-only native AI replay output transport, not production AI decoding.
mod common;
use std::path::Path;

fn require_sha256(path: &Path, expected: &str) {
    let output = std::process::Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("SHA256 utility required");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .next(),
        Some(expected)
    );
}

#[test]
#[ignore = "requires pinned RRRAH_AI_METAL_ROOT replay/source corpus and actual Metal"]
fn pinned_native_ai_planes_survive_managed_metal_display() {
    assert_eq!(std::env::var("RRRAH_GPU_BACKEND").as_deref(), Ok("metal"));
    let root =
        std::path::PathBuf::from(std::env::var_os("RRRAH_AI_METAL_ROOT").expect("AI corpus root required"));
    require_sha256(
        &root.join("ai-corpus/VectorApple.ai"),
        "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06",
    );
    require_sha256(
        &root.join("ai-corpus/one.ai"),
        "97786949c268233a446e1738fb9c2f941e4d109811bd2f972948cdb7a45d64b4",
    );
    let gpu = common::qualification_gpu().expect("actual Metal required");
    eprintln!("AI transport Metal adapter: {}", gpu.adapter_name());
    let mut checked = 0usize;
    for (name, width, height, hash) in [
        (
            "VectorApple.ai-page-0.native.rgba",
            301u32,
            246u32,
            "ecbb6bcbf532cb70c0e35be8c9d655c3cb38ddd8cfd85288e6e3c4c544020efc",
        ),
        (
            "one.ai-page-0.native.rgba",
            1366,
            768,
            "4d4dc1019b440c5389a7ed8731db277513f63d501006a3d97fc6a861e78e76c8",
        ),
        (
            "one.ai-page-1.native.rgba",
            177,
            175,
            "89db416a84fdc983853717f5fd73690c020e6feced867d1a3d4e02873f9d7e36",
        ),
    ] {
        let path = root.join("ai-native-finer-current-20261011").join(name);
        require_sha256(&path, hash);
        let count = width as usize * height as usize * 4;
        assert_eq!(std::fs::metadata(&path).unwrap().len(), count as u64);
        let cpu = rrrah_core::MemoryBudget::new(count as u64 * 5);
        let mut rgba = cpu.try_buffer(count, 0u8).unwrap();
        std::io::Read::read_exact(&mut std::fs::File::open(path).unwrap(), &mut rgba).unwrap();
        // Match the strict AI diagnostic's explicit white composite in sRGB8.
        // Geometry/color qualification is separate from GPU transport.
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
            pixel[3] = 255;
        }
        let raster = rrrah_core::DecodedRaster::new(
            width,
            height,
            rrrah_core::RasterPixels::Rgba8(rgba.freeze().into()),
            rrrah_core::RasterColorSpace::Srgb,
        )
        .unwrap();
        let linear = raster.to_linear_srgb_with_budget(Some(&cpu)).unwrap();
        let texture = rrrah_core::MemoryBudget::new(count as u64 * 8);
        let uploads = rrrah_core::MemoryBudget::new(count as u64 * 8);
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_with_budget(&gpu.device, common::READBACK_FORMAT, texture.clone())
                .with_upload_queue_budget(uploads.clone());
        renderer.upload(&gpu.device, &gpu.queue, &linear).unwrap();
        renderer.update_view(
            &gpu.queue,
            rrrah_gpu::ViewParameters {
                viewport: [width as f32, height as f32],
                zoom: 1.0
                    / (((width as f32 - 32.0) / width as f32).min((height as f32 - 32.0) / height as f32)),
                ..Default::default()
            },
        );
        let output = gpu.render_existing_raster(&renderer, [width, height]);
        let rrrah_core::RasterPixels::Rgba8(expected) = raster.pixels() else {
            unreachable!()
        };
        let maximum = output
            .pixels
            .iter()
            .zip(expected.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            maximum <= 1,
            "{name}: SDR transfer roundtrip exceeded one code: {maximum}"
        );
        assert!(output.pixels.chunks_exact(4).all(|p| p[3] == 255));
        checked += width as usize * height as usize;
        drop((renderer, linear, raster));
        assert_eq!((cpu.used(), texture.used(), uploads.used()), (0, 0, 0));
        eprintln!(
            "AI Metal plane: {name}; pixels={}; max_RGB8_error={maximum}; final_credit=0",
            width as usize * height as usize
        );
    }
    eprintln!("AI Metal transport: pages=3; checked_pixels={checked}; production_AI_fidelity_unqualified");
}
