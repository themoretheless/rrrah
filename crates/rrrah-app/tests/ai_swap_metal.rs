//! Test-only replay planes, not the production AI decoder.
#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

fn render_managed(gpu: &common::GpuReadback, raster: &rrrah_core::DecodedRaster) -> Vec<u8> {
    let bytes = u64::from(raster.width()) * u64::from(raster.height()) * 32;
    let texture = rrrah_core::MemoryBudget::new(bytes);
    let uploads = rrrah_core::MemoryBudget::new(bytes);
    let mut renderer =
        rrrah_gpu::RasterRenderer::new_with_budget(&gpu.device, common::READBACK_FORMAT, texture.clone())
            .with_upload_queue_budget(uploads.clone());
    renderer.upload(&gpu.device, &gpu.queue, raster).unwrap();
    let (w, h) = (raster.width() as f32, raster.height() as f32);
    renderer.update_view(
        &gpu.queue,
        rrrah_gpu::ViewParameters {
            viewport: [w, h],
            zoom: 1. / ((w - 32.) / w).min((h - 32.) / h),
            ..Default::default()
        },
    );
    let frame = gpu.render_existing_raster(&renderer, [raster.width(), raster.height()]);
    drop(renderer);
    assert_eq!((texture.used(), uploads.used()), (0, 0));
    frame.pixels
}

#[test]
#[ignore = "requires pinned RRRAH_AI_METAL_ROOT replay/source corpus and actual Metal"]
fn pinned_ai_ram_swap_pressure_retry_preserves_metal_frames() {
    assert_eq!(std::env::var("RRRAH_GPU_BACKEND").as_deref(), Ok("metal"));
    let root = std::path::PathBuf::from(std::env::var_os("RRRAH_AI_METAL_ROOT").expect("AI corpus required"));
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ai-metal-replay-planes.json"
    ))
    .unwrap();
    for (name, hash) in manifest["files"].as_object().unwrap() {
        let output = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(root.join(name))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            hash.as_str()
        );
    }
    let gpu = common::qualification_gpu().expect("actual Metal required");
    eprintln!("AI swap Metal adapter: {}", gpu.adapter_name());
    for (name, w, h) in [
        ("VectorApple.ai-page-0.native.rgba", 301u32, 246u32),
        ("one.ai-page-0.native.rgba", 1366, 768),
        ("one.ai-page-1.native.rgba", 177, 175),
    ] {
        let count = w as usize * h as usize * 4;
        let cpu = rrrah_core::MemoryBudget::new(count as u64 * 12);
        let mut bytes = cpu.try_buffer(count, 0u8).unwrap();
        std::io::Read::read_exact(
            &mut std::fs::File::open(root.join("ai-native-finer-current-20261011").join(name)).unwrap(),
            &mut bytes,
        )
        .unwrap();
        for pixel in bytes.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            for c in &mut pixel[..3] {
                *c = ((u32::from(*c) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
            pixel[3] = 255;
        }
        let native = rrrah_core::DecodedRaster::new(
            w,
            h,
            rrrah_core::RasterPixels::Rgba8(bytes.freeze().into()),
            rrrah_core::RasterColorSpace::Srgb,
        )
        .unwrap();
        let linear = native.to_linear_srgb_with_budget(Some(&cpu)).unwrap();
        let expected = render_managed(&gpu, &linear);
        let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
            max_bytes: linear.capacity_bytes(),
            max_entries: Some(1),
            ttl: None,
        });
        assert!(ram.insert(1, linear.clone()));
        let lease = ram.get_lease(&1).unwrap();
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(render_managed(&gpu, &lease), expected);
        drop(lease);
        drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
        drop(ram);
        for (kind, source) in [("srgb8", native), ("linear32", linear)] {
            let weight = source.capacity_bytes();
            let directory = tempfile::tempdir().unwrap();
            let queue = rrrah_core::MemoryBudget::new(weight);
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: weight * 2 + 65536,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: weight,
                    queue_count: 1,
                    restore_bytes: weight,
                },
                queue.clone(),
                cpu.clone(),
            )
            .unwrap();
            assert_eq!(
                swap.try_enqueue(1, source.clone()),
                rrrah_cache::SpillAdmission::Queued
            );
            swap.wait_idle().unwrap();
            let pressure = cpu.try_reserve(cpu.available_bytes()).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            drop(pressure);
            assert!(swap.try_get(&1, || true).unwrap().is_none());
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            assert_eq!(
                (restored.width(), restored.height(), restored.color_space()),
                (source.width(), source.height(), source.color_space())
            );
            match (restored.pixels(), source.pixels()) {
                (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) => {
                    assert_eq!(&a[..], &b[..])
                }
                (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) => {
                    assert_eq!(&a[..], &b[..])
                }
                _ => panic!("sample precision changed"),
            }
            let display = restored.to_linear_srgb_with_budget(Some(&cpu)).unwrap();
            assert_eq!(render_managed(&gpu, &display), expected);
            drop((display, restored, source));
            assert_eq!(
                (swap.stats().writes, swap.stats().reads, swap.stats().errors),
                (1, 1, 0)
            );
            drop(swap);
            assert_eq!(queue.used(), 0);
            eprintln!(
                "AI swap plane: {name}; kind={kind}; samples/frame exact; pressure/cancel retry; queue=0"
            );
        }
        assert_eq!(cpu.used(), 0);
    }
    eprintln!("AI Metal swap: pages=3; storage_cases=6; root=0; production_AI_fidelity_unqualified");
}
