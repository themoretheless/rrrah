#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use rrrah_gpu::ViewParameters;
use std::sync::Arc;

fn frame(width: u32, pixels: Vec<f32>) -> DecodedRaster {
    DecodedRaster::new(
        width,
        1,
        RasterPixels::Rgba32Float(Arc::new(pixels).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
}
fn view() -> ViewParameters {
    ViewParameters {
        viewport: [64.0; 2],
        zoom: 2.0,
        ..ViewParameters::default()
    }
}

#[test]
fn profiled_png_and_archives_keep_color_through_swap_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("ICC raster swap adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for name in [
        "pattern.profiled.png",
        "pattern.profiled.tif",
        "oriented.profiled.tif",
        "pattern.profiled.png.ora",
        "pattern.profiled.png.kra",
        "../xcf/profiled-normal-overlay-v1.xcf",
    ] {
        let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let RasterColorSpace::Icc(profile) = decoded.color_space() else {
            panic!("{name}: ICC absent")
        };
        assert!(!profile.is_empty());
        let expected_profile = profile.clone();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let source_weight = decoded.capacity_bytes();
        assert!(source_weight > decoded.pixel_capacity_bytes());
        assert_eq!(budget.used(), source_weight);
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
        let actual = gpu.render_raster(&prepared, parameters, [64, 64]);
        let (width, height, rgba) = if name == "oriented.profiled.tif" {
            (1, 2, vec![255, 0, 0, 255, 0, 255, 0, 128])
        } else if name == "../xcf/profiled-normal-overlay-v1.xcf" {
            (2, 1, vec![127, 128, 0, 255, 0, 0, 255, 255])
        } else {
            let golden_name = if name == "pattern.profiled.tif" {
                "pattern.profiled.png.rgba".to_owned()
            } else {
                format!("{name}.rgba")
            };
            (16, 16, std::fs::read(fixtures.join(golden_name)).unwrap())
        };
        let golden = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba8(Arc::new(rgba).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let expected = gpu.render_raster(&golden, parameters, [64, 64]);
        assert!(
            actual
                .pixels
                .iter()
                .zip(&expected.pixels)
                .all(|(a, b)| a.abs_diff(*b) <= 1),
            "{name}: ICC reference differs"
        );
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: source_weight,
                queue_count: 1,
                restore_bytes: source_weight,
            },
            rrrah_core::MemoryBudget::new(source_weight),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1, decoded);
        swap.wait_idle().unwrap();
        assert_eq!(budget.used(), prepared.capacity_bytes());
        drop(prepared);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(budget.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        assert_eq!(swap.stats().errors, 0);
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::Icc(expected_profile));
        assert_eq!(budget.used(), source_weight);
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        assert_eq!(
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels,
            actual.pixels,
            "{name}: restored color changed"
        );
        drop(restored);
        assert_eq!(budget.used(), prepared.capacity_bytes());
        drop(prepared);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
fn masked_legacy_xcf_preserves_explicit_color_assumption_through_swap_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Masked XCF adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xcf");
    for (name, expected) in [
        ("normal-mask-enabled-v1.xcf", [255, 0, 0, 255, 0, 128, 127, 255]),
        (
            "normal-mask-disabled-v1.xcf",
            [127, 128, 0, 255, 0, 128, 127, 255],
        ),
        (
            "normal-mask-negative-offset-v1.xcf",
            [127, 128, 0, 255, 0, 0, 255, 255],
        ),
        ("normal-mask-hidden-v1.xcf", [255, 0, 0, 255, 0, 0, 255, 255]),
        (
            "normal-mask-zero-opacity-v1.xcf",
            [255, 0, 0, 255, 0, 0, 255, 255],
        ),
        (
            "normal-mask-half-opacity-v1.xcf",
            [255, 0, 0, 255, 0, 64, 191, 255],
        ),
        ("normal-gray-v1.xcf", [17, 17, 17, 255, 239, 239, 239, 255]),
        ("normal-indexed-v1.xcf", [17, 43, 91, 255, 239, 181, 7, 255]),
        ("normal-indexed-alpha-v1.xcf", [17, 43, 91, 255, 239, 181, 7, 255]),
        (
            "normal-indexed-alpha-boundary-v1.xcf",
            [0, 0, 0, 0, 239, 181, 7, 255],
        ),
        ("normal-gray-alpha-v1.xcf", [17, 17, 17, 128, 239, 239, 239, 255]),
    ] {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
        request.memory_budget = Some(budget.clone());
        let strict = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(strict.color_space(), &RasterColorSpace::Unspecified);
        assert!(rrrah_decode::prepare_raster_for_display(&strict).is_err());
        drop(strict);
        assert_eq!(budget.used(), 0);
        request.assume_untagged_srgb = true;
        let raster = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(raster.color_space(), &RasterColorSpace::AssumedSrgb);
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&raster, Some(&budget)).unwrap();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let actual = gpu.render_raster(&prepared, parameters, [64, 64]);
        let golden = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba8(Arc::new(expected.to_vec()).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        assert_eq!(
            actual.pixels,
            gpu.render_raster(&golden, parameters, [64, 64]).pixels
        );
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: 8,
                queue_count: 1,
                restore_bytes: 8,
            },
            rrrah_core::MemoryBudget::new(8),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1, raster);
        swap.wait_idle().unwrap();
        drop(prepared);
        assert_eq!(budget.used(), 0);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::AssumedSrgb);
        let RasterPixels::Rgba8(pixels) = restored.pixels() else {
            panic!("wrong precision")
        };
        assert_eq!(&**pixels, &expected);
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        assert_eq!(
            actual.pixels,
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels
        );
        drop(restored);
        drop(prepared);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
fn corrupt_raster_swap_entries_release_memory_and_allow_replacement() {
    for kind in 0..3 {
        for truncate in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let budget = rrrah_core::MemoryBudget::new(128);
            let source = || {
                let pixels = match kind {
                    0 => RasterPixels::Rgba8(Arc::new(vec![255, 128, 0, 255]).into()),
                    1 => RasterPixels::Rgba16(Arc::new(vec![65535, 12345, 0, 65535]).into()),
                    _ => RasterPixels::Rgba32Float(
                        Arc::new(vec![16.0, -0.0, f32::from_bits(0x7fc01234), 1.0]).into(),
                    ),
                };
                DecodedRaster::new(1, 1, pixels, RasterColorSpace::LinearSrgb)
                    .unwrap()
                    .with_image_selection(2, 3)
                    .unwrap()
                    .try_manage_pixels(&budget)
                    .unwrap()
            };
            let mut expected = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected, &source()).unwrap();
            assert_eq!(budget.used(), 0);
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 4096,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: 128,
                    queue_count: 1,
                    restore_bytes: 128,
                },
                rrrah_core::MemoryBudget::new(128),
                budget.clone(),
            )
            .unwrap();
            swap.enqueue(1, source());
            swap.wait_idle().unwrap();
            assert_eq!(budget.used(), 0);
            let session = std::fs::read_dir(directory.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let blob = std::fs::read_dir(session)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let mut bytes = std::fs::read(&blob).unwrap();
            if truncate {
                bytes.pop();
            } else {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            std::fs::write(&blob, bytes).unwrap();
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().errors, 1);
            assert!(!blob.exists());
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(swap.stats().errors, 1);
            swap.enqueue(1, source());
            swap.wait_idle().unwrap();
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            let mut actual = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual, &restored).unwrap();
            assert_eq!(actual, expected, "kind {kind}, truncated {truncate}");
            drop(restored);
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().queued_bytes, 0);
        }
    }
}

#[test]
fn hdr_raster_swap_preserves_samples_and_linear_metal_output() {
    qualify_linear_hdr_swap(false);
}

#[test]
fn float_rla_swap_preserves_hdr_samples_and_linear_metal_output() {
    qualify_linear_hdr_swap(true);
}

fn qualify_linear_hdr_swap(float_rla: bool) {
    let weight = if float_rla { 32 } else { 16 };
    let instance = common::headless_instance();
    let adapter = match pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    )) {
        Ok(adapter) => adapter,
        Err(error) if std::env::var("RRRAH_GPU_OPTIONAL").as_deref() == Ok("1") => {
            eprintln!("HDR raster swap: GPU unavailable, optional skip: {error}");
            return;
        }
        Err(error) => panic!("HDR raster swap requires a GPU: {error}"),
    };
    eprintln!("HDR raster adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu_budget = rrrah_core::MemoryBudget::new(weight);
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, gpu_budget.clone());
    let budget = rrrah_core::MemoryBudget::new(if float_rla { 4096 } else { weight });
    let source = if float_rla {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        rrrah_decode::decode_rla_float_with_interpretation(
            &request,
            rrrah_decode::RlaFloatByteOrder::Little,
            rrrah_decode::RlaAlphaMode::Straight,
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
    } else {
        frame(1, vec![4.0, 2.0, -0.5, 1.0])
            .try_manage_pixels(&budget)
            .unwrap()
    };
    assert_eq!(budget.used(), weight);
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        budget.clone(),
    )
    .unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, source));
    let lease = ram.get_lease(&1).unwrap();
    assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    drop(lease);
    // Move the visible owner away so the old HDR frame becomes spillable.
    ram.mark_visible(2);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    assert!(ram.is_empty());
    let swap = ram.swap().unwrap();
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    assert_eq!(swap.stats().errors, 0);
    drop(pressure);
    let source = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(source.color_space(), &RasterColorSpace::LinearSrgb);
    let RasterPixels::Rgba32Float(samples) = source.pixels() else {
        panic!("HDR sample type changed")
    };
    let expected = if float_rla {
        vec![4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]
    } else {
        vec![4.0, 2.0, -0.5, 1.0]
    };
    assert_eq!(
        samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v: &f32| v.to_bits()).collect::<Vec<_>>()
    );
    assert!(samples.is_managed());
    assert_eq!(budget.used(), weight);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 1);
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().queued_bytes, 0);
    renderer.upload(&device, &queue, &source).unwrap();
    // Upload owns its GPU copy: CPU reservation can be released before drawing.
    drop(source);
    assert_eq!(budget.used(), 0);
    let mut parameters = view();
    // Fit leaves a 32-pixel viewport margin; the 2:1 producer fixture needs
    // zoom four to cover the entire square target, including its top row.
    if float_rla {
        parameters.zoom = 4.0;
    }
    parameters.exposure_stops = 1.0;
    renderer.update_view(&queue, parameters);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear HDR readback target"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 512 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512),
                rows_per_image: Some(64),
            },
        },
        wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    // Exact IEEE binary16 encodings: 8, 4, -1, opaque alpha.
    for (index, pixel) in mapped.chunks_exact(8).enumerate() {
        let channels: Vec<u16> = pixel
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        assert_eq!(channels, [0x4800, 0x4400, 0xbc00, 0x3c00], "HDR pixel {index}");
    }
    drop(mapped);
    readback.unmap();
    assert_eq!(gpu_budget.used(), weight);
    drop(renderer);
    assert_eq!(gpu_budget.used(), 0);
}

#[test]
fn pict_public_route_lease_swap_pressure_and_metal_preserve_rgba() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pict/red-rectangle-v2.pict");
    qualify_pict_transport(path, 4096, None);
}

#[test]
#[ignore = "requires actual GPU and external PICT/TwelveMonkeys corpus"]
fn pict_external_directbits_oracle_swap_pressure_and_metal() {
    let root = std::env::var_os("RRRAH_PICT_CORPUS").expect("RRRAH_PICT_CORPUS required");
    for name in ["mire16.pict", "mire32.pict", "16bit.pict", "32bit.pict"] {
        let root = std::path::Path::new(&root);
        let oracle = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
        qualify_pict_transport(root.join(name), 4 * 1024 * 1024, Some(&oracle));
    }
}

#[test]
fn pict_unpacked_xrgb_macos_pixels_survive_swap_pressure_and_metal() {
    // Recorded macOS sips RGBA; qualifier pins the fixture hash and manifest.
    let oracle = [255, 0, 0, 255, 17, 95, 203, 255];
    for name in ["unpacked-xrgb.pict", "drop-pad-rgb.pict"] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/pict")
            .join(name);
        qualify_pict_transport(path, 4096, Some(&oracle));
    }
}

fn render_hdr_float_metal(source: &DecodedRaster) -> Vec<u16> {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("HDR float adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu_budget = rrrah_core::MemoryBudget::new(source.pixel_capacity_bytes());
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, gpu_budget.clone());
    renderer.upload(&device, &queue, source).unwrap();
    let parameters = ViewParameters {
        viewport: [128.0, 64.0],
        zoom: 2.0,
        exposure_stops: 1.0,
        ..view()
    };
    renderer.update_view(&queue, parameters);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear HDR readback target"),
        size: wgpu::Extent3d {
            width: 128,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 1024 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(64),
            },
        },
        wgpu::Extent3d {
            width: 128,
            height: 64,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    let result: Vec<u16> = mapped
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect();
    drop(mapped);
    readback.unmap();
    drop(renderer);
    assert_eq!(gpu_budget.used(), 0);
    result
}

#[test]
fn pq_avif_linear_pixels_survive_pressure_swap_and_metal() {
    for name in ["avif-pq10-svt.avif", "avif-pq10-color-svt.avif"] {
        qualify_hdr_avif_transport(name);
    }
}

#[test]
fn hlg_avif_linear_pixels_survive_pressure_swap_and_metal() {
    for name in ["avif-hlg10-svt.avif", "avif-hlg10-color-svt.avif"] {
        qualify_hdr_avif_transport(name);
    }
}

fn qualify_hdr_avif_transport(name: &str) {
    let hlg = name.contains("hlg");
    let gpu = common::qualification_gpu().expect("HDR AVIF transport requires a GPU");
    eprintln!("HDR AVIF adapter: {}", gpu.adapter_name());
    let budget = rrrah_core::MemoryBudget::new(512 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster")
            .join(name),
    );
    request.memory_budget = Some(budget.clone());
    request.pq_reference_white_nits = if hlg { None } else { Some(100.) };
    request.hlg_display = if hlg { Some((100., 1000., 1.2)) } else { None };
    let source = rrrah_decode::decode_raster(&request).unwrap();
    assert_eq!(source.color_space(), &RasterColorSpace::LinearSrgb);
    let RasterPixels::Rgba32Float(pixels) = source.pixels() else {
        panic!()
    };
    let expected: Vec<u32> = pixels.iter().map(|v| v.to_bits()).collect();
    assert!(pixels.chunks_exact(4).any(|p| p[0] > if hlg { 9.5 } else { 95. }));
    let float_before = render_hdr_float_metal(&source);
    assert!(
        float_before
            .chunks_exact(4)
            .any(|p| p[0] > if hlg { 0x4c00 } else { 0x5800 })
    );
    assert!(float_before.chunks_exact(4).all(|p| p[3] == 0x3c00));
    if name.contains("color") {
        assert!(
            float_before
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|v| *v & 0x8000 != 0 && *v & 0x7fff != 0))
        );
    }
    let peak_bits = float_before
        .chunks_exact(4)
        .map(|p| p[0])
        .filter(|v| v & 0x8000 == 0)
        .max()
        .unwrap();
    assert!(peak_bits & 0x8000 == 0 && peak_bits & 0x7c00 != 0x7c00);
    let peak = f64::from(1024 + (peak_bits & 1023)) * 2f64.powi(i32::from((peak_bits >> 10) & 31) - 25);
    let cpu_peak = pixels.chunks_exact(4).map(|p| p[0]).fold(0f32, f32::max);
    assert!((peak - f64::from(cpu_peak) * 2.0).abs() <= 0.25);
    let weight = source.capacity_bytes();
    assert_eq!(weight, 128 * 64 * 16);
    let parameters = ViewParameters { zoom: 1.0, ..view() };
    let before = gpu.render_raster(&source, parameters, [128, 64]);
    assert!(
        before
            .pixels
            .chunks_exact(4)
            .any(|p| p[..3] != before.pixels[..3])
    );
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 1024 * 1024,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        budget.clone(),
    )
    .unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, source));
    let lease = ram.get_lease(&1).unwrap();
    assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    drop(lease);
    ram.mark_visible(2);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    let swap = ram.swap().unwrap();
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), &RasterColorSpace::LinearSrgb);
    assert_eq!((restored.width(), restored.height()), (128, 64));
    let RasterPixels::Rgba32Float(pixels) = restored.pixels() else {
        panic!()
    };
    assert_eq!(pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    assert_eq!(budget.used(), weight);
    let float_after = render_hdr_float_metal(&restored);
    assert_eq!(float_before, float_after);
    let after = gpu.render_raster(&restored, parameters, [128, 64]);
    assert_eq!(before.pixels, after.pixels);
    let alias = restored.clone();
    drop(restored);
    assert_eq!(budget.used(), weight);
    drop(alias);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 1);
    assert_eq!(swap.stats().errors, 0);
}

#[test]
fn avif_clean_aperture_ram_swap_and_metal_preserve_pixels() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut names = vec!["avif-clean-aperture-integer.avif".to_owned()];
    names.extend((2..=8).map(|i| format!("avif-clean-aperture-orientation-{i}.avif")));
    names.push("avif-clean-aperture-top-left-integer.avif".to_owned());
    names.extend((2..=8).map(|i| format!("avif-clean-aperture-top-left-orientation-{i}.avif")));
    names.push("avif-libavif-crop-rgb.avif".to_owned());
    names.extend((2..=8).map(|i| format!("avif-libavif-crop-orientation-{i}.avif")));
    for name in names {
        let path = root.join(&name);
        let decoded = rrrah_decode::decode_raster(&rrrah_decode::DecodeRequest::new(&path)).unwrap();
        let dimensions = if name == "avif-libavif-crop-rgb.avif" {
            (8, 8)
        } else if name.contains("integer") {
            (8, 6)
        } else if ["-5.avif", "-6.avif", "-7.avif", "-8.avif"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
        {
            (2, 4)
        } else {
            (4, 2)
        };
        assert_eq!((decoded.width(), decoded.height()), dimensions);
        assert_eq!(decoded.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(pixels) = decoded.pixels() else {
            panic!()
        };
        let oracle = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
        assert_eq!(pixels.len(), oracle.len());
        assert!(pixels.iter().zip(&oracle).all(|(a, b)| a.abs_diff(*b) <= 3));
        let actual = pixels.to_vec();
        drop(decoded);
        qualify_u8_raster_transport(path, 4096, Some(&actual), RasterColorSpace::Srgb);
    }
}

fn qualify_pict_transport(path: std::path::PathBuf, limit: u64, oracle: Option<&[u8]>) {
    qualify_u8_raster_transport(path, limit, oracle, RasterColorSpace::AssumedSrgb);
}

fn qualify_u8_raster_transport(
    path: std::path::PathBuf,
    limit: u64,
    oracle: Option<&[u8]>,
    color: RasterColorSpace,
) {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("Raster transport adapter: {}", gpu.adapter_name());
    let budget = rrrah_core::MemoryBudget::new(limit);
    let mut request = rrrah_decode::DecodeRequest::new(path);
    request.memory_budget = Some(budget.clone());
    request.assume_untagged_srgb = true;
    let rrrah_decode::DecodedImage::Raster(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!()
    };
    if let Some(oracle) = oracle {
        let RasterPixels::Rgba8(p) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(&**p, oracle);
    }
    let dimensions = (decoded.width(), decoded.height());
    let weight = decoded.capacity_bytes();
    let expected = decoded.clone();
    let mut ram = rrrah_cache::LeaseCache::new(rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: None,
    });
    ram.insert(1u8, decoded, weight).unwrap();
    let lease = ram.get(&1).unwrap();
    assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
    let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&lease, Some(&budget)).unwrap();
    let parameters = view();
    let before = gpu.render_raster(&prepared, parameters, [64, 64]).pixels;
    if oracle.is_some() {
        assert!(
            before.chunks_exact(4).any(|pixel| pixel != &before[..4]),
            "oracle-qualified raster GPU frame must contain image detail"
        );
    }
    drop(prepared);
    drop(lease);
    let (_, victim) = ram.take_lru().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: limit,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        budget.clone(),
    )
    .unwrap();
    swap.enqueue(1, victim);
    swap.wait_idle().unwrap();
    // The independent comparison retains source allocation until explicitly dropped.
    let expected_pixels = match expected.pixels() {
        RasterPixels::Rgba8(p) => p.to_vec(),
        _ => panic!(),
    };
    drop(expected);
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_buffer(limit as usize, 0u8).unwrap().freeze();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    // The local restore cap must bind even while the shared parent has room.
    assert!(budget.available_bytes() >= weight);
    assert!(matches!(swap.try_get(&1,||false),
        Err(rrrah_core::BufferError::Capacity {limit,..}) if limit<=weight));
    let retained = restored.clone();
    assert_eq!((restored.width(), restored.height()), dimensions);
    assert_eq!(restored.color_space(), &color);
    let RasterPixels::Rgba8(p) = restored.pixels() else {
        panic!()
    };
    assert_eq!(&**p, &expected_pixels);
    if let Some(oracle) = oracle {
        assert_eq!(&**p, oracle);
    }
    let ready = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
    assert_eq!(gpu.render_raster(&ready, parameters, [64, 64]).pixels, before);
    drop(ready);
    drop(restored);
    assert_eq!(budget.used(), weight);
    assert!(matches!(swap.try_get(&1,||false),
        Err(rrrah_core::BufferError::Capacity {limit,..}) if limit<=weight));
    drop(retained);
    assert_eq!(budget.used(), 0);
    let retry = swap.try_get(&1, || false).unwrap().unwrap();
    let RasterPixels::Rgba8(p) = retry.pixels() else {
        panic!()
    };
    assert_eq!(&**p, &expected_pixels);
    drop(retry);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 2);
}
