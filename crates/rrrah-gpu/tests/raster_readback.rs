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
fn split_comparison_reuses_resident_pixels_and_matches_both_reference_exposures() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let source = frame(1, vec![0.18, 0.36, 4., 0.5]);
    eprintln!("split comparison adapter: {}", gpu.adapter_name());
    for format in [common::READBACK_FORMAT, wgpu::TextureFormat::Rgba16Float] {
        let budget = rrrah_core::MemoryBudget::new(16);
        let mut renderer = if format == wgpu::TextureFormat::Rgba16Float {
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone())
        } else {
            rrrah_gpu::RasterRenderer::new_with_budget(&gpu.device, format, budget.clone())
        };
        renderer.upload(&gpu.device, &gpu.queue, &source).unwrap();
        let render = |renderer: &rrrah_gpu::RasterRenderer| -> Vec<u8> {
            if format == wgpu::TextureFormat::Rgba16Float {
                gpu.render_existing_raster_half(renderer, [64, 64])
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect()
            } else {
                gpu.render_existing_raster(renderer, [64, 64]).pixels
            }
        };
        renderer.update_view(&gpu.queue, view());
        let before = render(&renderer);
        renderer.update_view(
            &gpu.queue,
            ViewParameters {
                exposure_stops: 1.,
                ..view()
            },
        );
        let after = render(&renderer);
        assert_ne!(before, after);
        renderer.set_comparison(&gpu.queue, 0.5, 0.);
        let split = render(&renderer);
        let bytes_per_pixel = if format == wgpu::TextureFormat::Rgba16Float {
            8
        } else {
            4
        };
        for index in 0..4096 {
            let offset = index * bytes_per_pixel;
            let reference = if index % 64 < 32 { &before } else { &after };
            assert_eq!(
                &split[offset..offset + bytes_per_pixel],
                &reference[offset..offset + bytes_per_pixel]
            );
        }
        assert_eq!(
            budget.used(),
            16,
            "comparison must not allocate another pixel texture"
        );
        renderer.set_comparison(&gpu.queue, 0., 0.);
        assert_eq!(render(&renderer), after);
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
#[ignore = "Metal-specific empirical range edge; outside portable binary16 caller contract"]
fn hdr_exposure_saturates_to_finite_binary16_range() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let source = frame(1, vec![65504., -65504., 0.5, 1.]);
    eprintln!("HDR finite-range adapter: {}", gpu.adapter_name());
    for (exposure_stops, expected) in [
        (-10., [0x53ff, 0xd3ff, 0x1000, 0x3c00]),
        (0., [0x7bff, 0xfbff, 0x3800, 0x3c00]),
        (10., [0x7bff, 0xfbff, 0x6000, 0x3c00]),
    ] {
        let output = gpu.render_raster_half(
            &source,
            ViewParameters {
                exposure_stops,
                ..view()
            },
            [64, 64],
        );
        assert_eq!(output.len(), 64 * 64 * 4);
        for pixel in output.chunks_exact(4) {
            assert_eq!(pixel, expected, "exposure={exposure_stops}");
        }
        eprintln!(
            "HDR finite-range: exposure={exposure_stops}; all4096 pixels exact; finite signed binary16"
        );
    }
}

#[test]
#[ignore = "Metal-specific empirical range edge; outside portable binary16 caller contract"]
fn hdr_opposite_extreme_samples_interpolate_without_nan() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("HDR opposite-extreme adapter: {}", gpu.adapter_name());
    let source = frame(2, vec![f32::MAX, f32::MAX, 0.5, 1., -f32::MAX, f32::MAX, 0.5, 1.]);
    let output = gpu.render_raster_half(
        &source,
        ViewParameters {
            viewport: [63., 65.],
            zoom: 2.,
            ..Default::default()
        },
        [63, 65],
    );
    let center = (32 * 63 + 31) * 4;
    assert_eq!(&output[center..center + 4], &[0, 0x7bff, 0x3800, 0x3c00]);
    assert!(
        output.iter().all(|v| v & 0x7c00 != 0x7c00),
        "no Inf or NaN in HDR target"
    );
    eprintln!("HDR opposite-extreme: 63x65; symmetric center exact; all components finite");
}

#[test]
fn linear_gray_and_straight_alpha_composite_match_display_reference() {
    let Some(gpu) = common::qualification_gpu() else {
        eprintln!("raster readback: no adapter; skipped");
        return;
    };
    eprintln!("raster readback adapter: {}", gpu.adapter_name());
    let output = gpu.render_raster(
        &frame(1, vec![0.214_041_14, 0.214_041_14, 0.214_041_14, 1.0]),
        view(),
        [64, 64],
    );
    assert!(
        output.max_channel_deviation([128, 128, 128, 255]) <= 1,
        "{:?}",
        output.center()
    );
    let output = gpu.render_raster(&frame(1, vec![1.0, 0.0, 0.0, 0.5]), view(), [64, 64]);
    assert!(
        output.max_channel_deviation([189, 24, 24, 255]) <= 2,
        "{:?}",
        output.center()
    );
}

#[test]
fn transparent_rgb_is_removed_before_bilinear_interpolation() {
    let Some(gpu) = common::qualification_gpu() else {
        eprintln!("raster readback: no adapter; skipped");
        return;
    };
    let output = gpu.render_raster(
        &frame(2, vec![1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0]),
        view(),
        [64, 64],
    );
    let center = output.center();
    assert!(center[0] < 40, "invisible red leaked: {center:?}");
    assert!(center[2] > 150, "blue coverage lost: {center:?}");
    assert_eq!(center[3], 255);
}

#[test]
fn uploaded_rows_and_rgb_channels_keep_their_positions() {
    let Some(gpu) = common::qualification_gpu() else {
        eprintln!("raster readback: no adapter; skipped");
        return;
    };
    let raster = DecodedRaster::new(
        2,
        2,
        RasterPixels::Rgba32Float(
            Arc::new(vec![
                1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
            ])
            .into(),
        ),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let result = gpu.render_raster(&raster, view(), [64, 64]);
    assert_eq!(result.pixel(0, 0), [255, 0, 0, 255]);
    assert_eq!(result.pixel(63, 0), [0, 255, 0, 255]);
    assert_eq!(result.pixel(0, 63), [0, 0, 255, 255]);
    assert_eq!(result.pixel(63, 63), [255, 255, 255, 255]);
}

#[test]
fn linear_hdr_attachment_preserves_bright_negative_and_exposed_channels() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("HDR raster adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu_budget = rrrah_core::MemoryBudget::new(16);
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, gpu_budget.clone());
    let budget = rrrah_core::MemoryBudget::new(16);
    let source = frame(1, vec![4.0, 2.0, -0.5, 1.0])
        .try_manage_pixels(&budget)
        .unwrap();
    assert_eq!(budget.used(), 16);
    renderer.upload(&device, &queue, &source).unwrap();
    // Upload owns its GPU copy: CPU reservation can be released before drawing.
    drop(source);
    assert_eq!(budget.used(), 0);
    let mut parameters = view();
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
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 32, y: 32, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
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
    let channels: &[u16] = bytemuck::cast_slice(&mapped[..8]);
    // Exact IEEE binary16 encodings: 8, 4, -1, opaque alpha.
    assert_eq!(channels, &[0x4800, 0x4400, 0xbc00, 0x3c00]);
    drop(mapped);
    readback.unmap();
    assert_eq!(gpu_budget.used(), 16);
    drop(renderer);
    assert_eq!(gpu_budget.used(), 0);
}

#[test]
fn texture_budget_preserves_loaded_image_on_refusal_and_releases_owners() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("texture budget adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(32);
    let mut renderer =
        rrrah_gpu::RasterRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    renderer.upload(&device, &queue, &frame(1, vec![1.0; 4])).unwrap();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 16);
    assert!(matches!(
        renderer.upload(&device, &queue, &frame(2, vec![1.0; 8])),
        Err(rrrah_gpu::RasterUploadError::Memory(_))
    ));
    assert!(renderer.has_image());
    assert_eq!(renderer.resident_bytes(), 16);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 16);
    renderer.upload(&device, &queue, &frame(1, vec![0.5; 4])).unwrap();
    assert_eq!(budget.peak(), 32);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 16);
    renderer.clear_image();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
    renderer.upload(&device, &queue, &frame(2, vec![1.0; 8])).unwrap();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 32);
    drop(renderer);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
}

#[test]
fn developed_raw_exposure_precedes_tone_and_monotone_curve() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    fn byte(x: f32) -> u8 {
        let s = if x <= 0.0031308 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        (s.clamp(0.0, 1.0) * 255.0).round() as u8
    }
    for curve in [
        rrrah_core::develop::MonotoneCurve::identity(),
        rrrah_core::develop::MonotoneCurve::new(&[(0.0, 0.0), (0.5, 0.3), (1.0, 1.0)]).unwrap(),
    ] {
        for stops in [-1.0, 0.0, 1.0] {
            let mut v = view();
            v.exposure_stops = stops;
            let source = [0.18_f32, 0.18, 0.18];
            let mapped = rrrah_core::aces_tone_map_rgb(source.map(|x| x * stops.exp2()));
            let expected = byte(curve.eval(f64::from(mapped[0])) as f32);
            let result =
                gpu.render_developed_raw(&frame(1, vec![0.18, 0.18, 0.18, 1.0]), v, [64, 64], &curve);
            assert!(
                result.max_channel_deviation([expected, expected, expected, 255]) <= 2,
                "{stops}: {:?}, expected {expected}",
                result.center()
            );
        }
    }
    // Closely spaced near-black knots must use exact Hermite evaluation,
    // rather than a coarse uniform LUT that smears the first segment.
    let curve = rrrah_core::develop::MonotoneCurve::new(&[(0.0, 0.0), (0.001, 0.4), (1.0, 1.0)]).unwrap();
    for value in [0.001_f32, 0.002, 0.005] {
        let mapped = rrrah_core::aces_tone_map_rgb([value; 3]);
        let expected = byte(curve.eval(f64::from(mapped[0])) as f32);
        let result = gpu.render_developed_raw(
            &frame(1, vec![value, value, value, 1.0]),
            view(),
            [64, 64],
            &curve,
        );
        assert!(
            result.max_channel_deviation([expected, expected, expected, 255]) <= 2,
            "{:?}, expected {expected}",
            result.center()
        );
    }
    // A camera color outside the sRGB gamut follows the same desaturation as RAW.
    let source = [-0.1, 0.3, 0.2];
    let expected = rrrah_core::aces_tone_map_rgb(source).map(byte);
    let result = gpu.render_developed_raw(
        &frame(1, vec![source[0], source[1], source[2], 1.0]),
        view(),
        [64, 64],
        &rrrah_core::develop::MonotoneCurve::identity(),
    );
    assert!(result.max_channel_deviation([expected[0], expected[1], expected[2], 255]) <= 2);
}

#[test]
fn raster_queue_admission_accounts_alignment_and_completion_ownership() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("GPU required for queue admission");
    eprintln!("raster queue adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let texture = rrrah_core::MemoryBudget::new(1024);
    let tight = rrrah_core::MemoryBudget::new(511);
    let mut renderer =
        rrrah_gpu::RasterRenderer::new_with_budget(&device, common::READBACK_FORMAT, texture.clone());
    renderer.upload(&device, &queue, &frame(1, vec![1.0; 4])).unwrap();
    renderer = renderer.with_upload_queue_budget(tight.clone());
    let hdr = frame(17, [4.0, -0.5, 2.0, 1.0].repeat(17));
    assert!(matches!(
        renderer.upload(&device, &queue, &hdr),
        Err(rrrah_gpu::RasterUploadError::Memory(_))
    ));
    assert_eq!(tight.peak(), 0);
    assert_eq!(texture.used(), 16);
    assert_eq!(renderer.resident_bytes(), 16);
    assert!(renderer.has_image());
    let queued = rrrah_core::MemoryBudget::new(512);
    renderer = renderer.with_upload_queue_budget(queued.clone());
    renderer.upload(&device, &queue, &hdr).unwrap();
    assert_eq!(queued.peak(), 512);
    assert_eq!(renderer.resident_bytes(), 272);
    drop(renderer);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(queued.used(), 0);
    assert_eq!(texture.used(), 0);
}

#[test]
#[ignore = "requires qualified X3F linear sRGB PPM and a physical GPU"]
fn qualified_sd10_linear_raster_full_frame_display_readback() {
    let path = std::env::var_os("RRRAH_X3F_LINEAR_SRGB_ORACLE").expect("qualified linear sRGB fixture");
    let bytes = std::fs::read(path).unwrap();
    let dimensions = std::env::var("RRRAH_X3F_EXPECTED_DIMENSIONS").unwrap_or_else(|_| "2267x1513".into());
    let (width, height) = dimensions.split_once('x').unwrap();
    let (width, height) = (width.parse::<u32>().unwrap(), height.parse::<u32>().unwrap());
    assert!(width > 32 && height > 32);
    let count = usize::try_from(u64::from(width) * u64::from(height)).unwrap();
    let header = format!("P6\n{width} {height}\n65535\n");
    assert!(bytes.starts_with(header.as_bytes()));
    assert_eq!(bytes.len() - header.len(), count * 6);
    let values: Vec<u16> = bytes[header.len()..]
        .chunks_exact(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(values.len(), count * 3);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut rgba = budget.try_buffer(count * 4, 0u16).unwrap();
    for (source, target) in values.chunks_exact(3).zip(rgba.chunks_exact_mut(4)) {
        target[..3].copy_from_slice(source);
        target[3] = 65535;
    }
    let raster = DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba16(rgba.freeze().into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let linear = raster
        .to_linear_srgb_with_budget_and_cancel(Some(&budget), || false)
        .unwrap();
    let gpu = common::qualification_gpu().expect("physical GPU required; no silent skip");
    eprintln!("X3F {dimensions} raster adapter: {}", gpu.adapter_name());
    let output = gpu.render_raster(
        &linear,
        ViewParameters {
            viewport: [width as f32, height as f32],
            zoom: height as f32 / (height as f32 - 32.0),
            ..Default::default()
        },
        [width, height],
    );
    assert_eq!(output.pixels.len(), count * 4);
    let mut maximum = 0u8;
    for (source, pixel) in values.chunks_exact(3).zip(output.pixels.chunks_exact(4)) {
        assert_eq!(pixel[3], 255);
        for c in 0..3 {
            let value = f32::from(source[c]) / 65535.0;
            let encoded = if value <= 0.0031308 {
                12.92 * value
            } else {
                1.055 * value.powf(1.0 / 2.4) - 0.055
            };
            let expected = (encoded.clamp(0.0, 1.0) * 255.0).round() as u8;
            maximum = maximum.max(pixel[c].abs_diff(expected));
        }
    }
    assert!(maximum <= 2, "full-frame sRGB deviation {maximum}");
    drop(linear);
    drop(raster);
    assert_eq!(budget.used(), 0);
    eprintln!("X3F {dimensions} full-frame GPU sRGB max deviation={maximum}; CPU leases released");
}

#[test]
fn pixel_aspect_rejection_is_atomic_and_next_upload_resets_geometry() {
    let gpu = common::qualification_gpu().expect("actual GPU required for aspect transition");
    eprintln!("aspect transition adapter: {}", gpu.adapter_name());
    let raster = frame(2, vec![1.0; 8]);
    let parameters = ViewParameters {
        viewport: [128.0, 64.0],
        ..Default::default()
    };
    let square = gpu.render_raster(&raster, parameters, [128, 64]);
    let expanded = gpu.render_raster_with_aspect(&raster, parameters, [128, 64], 2.0);
    assert_ne!(expanded.pixels, square.pixels);
    let (before, after) = gpu.render_raster_aspect_transition(
        &raster,
        parameters,
        [128, 64],
        2.0,
        &[0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX],
    );
    assert_eq!(before.pixels, expanded.pixels);
    assert_eq!(after.pixels, square.pixels);
}

#[test]
fn cancelled_upload_validation_and_admission_preserve_visible_texture() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("upload cancellation adapter: {}", gpu.adapter_name());
    let first = frame(1, vec![1., 0., 0., 1.]);
    let replacement = DecodedRaster::new(
        100,
        100,
        RasterPixels::Rgba32Float(Arc::new([0., 0., 1., 1.].repeat(10_000)).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    gpu.verify_cancelled_raster_upload_keeps_previous_frame(
        &first,
        &replacement,
        view(),
        [64, 64],
        [[0x3c00, 0, 0, 0x3c00], [0, 0, 0x3c00, 0x3c00]],
    );
}

#[test]
fn gpu_resident_compute_rows_match_cpu_exposure_reference() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("resident raster adapter: {}", gpu.adapter_name());
    for [width, height] in [[7, 3], [16, 3], [64, 257], [7, 257]] {
        let count = width * height;
        let pixels: Vec<[f32; 4]> = (0..count)
            .map(|i| {
                [
                    i as f32 / count as f32,
                    0.25,
                    (count - 1 - i) as f32 / count as f32,
                    if i % 3 == 0 { 0.5 } else { 1. },
                ]
            })
            .collect();
        let expected = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(
                Arc::new(
                    pixels
                        .iter()
                        .flat_map(|p| [p[0] * 2., p[1] * 2., p[2] * 2., p[3]])
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let a = gpu.render_raster(&expected, view(), [64, 64]);
        let b = gpu.render_resident_exposure(&pixels, [width, height], 1., view(), [64, 64]);
        assert_eq!(a.pixels, b.pixels);
        for aspect in [0.5, 2.0] {
            let expected = expected.clone().with_pixel_aspect(Some(aspect)).unwrap();
            let a = gpu.render_raster(&expected, view(), [64, 64]);
            let b = gpu.render_resident_exposure_with_aspect(
                &pixels,
                [width, height],
                1.,
                view(),
                [64, 64],
                Some(aspect),
            );
            assert_eq!(a.pixels, b.pixels);
        }
    }
}

#[test]
#[ignore = "requires production HERO9 scene-linear dump; transport qualification, not demosaic/color oracle"]
fn hero9_full_frame_developed_raw_matches_metal_display_transport() {
    let path = std::env::var("RRRAH_HERO9_LINEAR_DUMP").expect("RRRAH_HERO9_LINEAR_DUMP");
    qualify_full_gpr_display(std::path::Path::new(&path), [5568, 4176], "HERO9");
}

#[test]
#[ignore = "requires corrected production Fusion linear dumps; display transport qualification"]
fn fusion_back_full_frame_developed_raw_matches_metal_display_transport() {
    let root = std::env::var_os("RRRAH_FUSION_LINEAR_CORPUS").expect("RRRAH_FUSION_LINEAR_CORPUS");
    qualify_full_gpr_display(
        &std::path::PathBuf::from(root).join("Fusion-back.rgba32fle"),
        [3104, 3000],
        "Fusion-back",
    );
}

#[test]
#[ignore = "requires corrected production Fusion linear dumps; display transport qualification"]
fn fusion_front_full_frame_developed_raw_matches_metal_display_transport() {
    let root = std::env::var_os("RRRAH_FUSION_LINEAR_CORPUS").expect("RRRAH_FUSION_LINEAR_CORPUS");
    qualify_full_gpr_display(
        &std::path::PathBuf::from(root).join("Fusion-front.rgba32fle"),
        [3104, 3000],
        "Fusion-front",
    );
}

fn qualify_full_gpr_display(path: &std::path::Path, extent: [u32; 2], name: &str) {
    let bytes = std::fs::read(path).unwrap();
    let [width, height] = extent;
    assert_eq!(bytes.len(), width as usize * height as usize * 16);
    let pixels: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    drop(bytes);
    assert!(pixels.iter().all(|v| v.is_finite()));
    let raster = DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(Arc::new(pixels).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let gpu = common::qualification_gpu().expect("required Metal adapter");
    eprintln!("{name} full-frame display adapter: {}", gpu.adapter_name());
    let output = gpu.render_developed_raw(
        &raster,
        ViewParameters {
            viewport: [width as f32, height as f32],
            zoom: height as f32 / (height - 32) as f32,
            ..Default::default()
        },
        [width, height],
        &rrrah_core::develop::MonotoneCurve::identity(),
    );
    let RasterPixels::Rgba32Float(source) = raster.pixels() else {
        unreachable!()
    };
    let mut maximum = 0_u8;
    for (i, (input, actual)) in source
        .chunks_exact(4)
        .zip(output.pixels.chunks_exact(4))
        .enumerate()
    {
        let expected = common::cpu_reference_rgb([input[0] as f64, input[1] as f64, input[2] as f64]);
        for c in 0..3 {
            maximum = maximum.max(actual[c].abs_diff(expected[c]));
        }
        assert_eq!(actual[3], 255, "alpha pixel {i}");
    }
    eprintln!(
        "{name} pixels checked: {}; maximum RGB8 difference: {maximum}",
        width as u64 * height as u64
    );
    assert!(maximum <= 2, "full-frame transport difference {maximum}");
}

#[test]
#[ignore = "manual 12 MP Metal upload cancellation qualification"]
fn large_cancelled_upload_validation_and_admission_preserve_visible_texture() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("large upload cancellation adapter: {}", gpu.adapter_name());
    let first = frame(1, vec![4., -0.5, 0., 1.]);
    let replacement = DecodedRaster::new(
        4000,
        3000,
        RasterPixels::Rgba32Float(Arc::new([-0.25, 0., 4., 1.].repeat(12_000_000)).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    gpu.verify_cancelled_raster_upload_keeps_previous_frame(
        &first,
        &replacement,
        ViewParameters {
            viewport: [128.0, 96.0],
            ..view()
        },
        [128, 96],
        [[0x4400, 0xb800, 0, 0x3c00], [0xb400, 0, 0x4400, 0x3c00]],
    );
}

#[test]
fn tiled_raster_seams_padding_and_partial_cancel_preserve_sdr_hdr() {
    let reference = common::qualification_gpu().expect("actual GPU required");
    let mut tiled = common::qualification_gpu().expect("actual GPU required");
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    tiled.device = device;
    tiled.queue = queue;
    eprintln!(
        "tiled raster adapter: {}; negotiated extent64",
        tiled.adapter_name()
    );
    for (width, height) in [(130u32, 5u32), (5, 130), (130, 130)] {
        let pixels: Vec<f32> = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    [
                        ((x % 7) as f32 - 2.) / 3.,
                        (y % 5) as f32 / 2.,
                        1.5,
                        [0., 0.25, 0.5, 1.][((x + y) % 4) as usize],
                    ]
                })
            })
            .collect();
        let raster = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let columns = width.div_ceil(64);
        let rows = height.div_ceil(64);
        let tw = width.div_ceil(columns);
        let th = height.div_ceil(rows);
        let bytes = u64::from(tw) * u64::from(th) * u64::from(columns * rows) * 16;
        assert_eq!(
            rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&tiled.device, width, height).unwrap(),
            bytes
        );
        let fit = (32. / width as f32).min(32. / height as f32);
        for (x, y) in [
            (tw as f32, th as f32),
            ((tw * 2) as f32, (th * 2) as f32),
            (width as f32 / 2., height as f32 / 2.),
        ] {
            let view = ViewParameters {
                viewport: [64., 64.],
                zoom: 1. / fit,
                pan: [width as f32 / 2. - x + 0.5, height as f32 / 2. - y + 0.5],
                ..Default::default()
            };
            assert_eq!(
                reference.render_raster(&raster, view, [64, 64]).pixels,
                tiled.render_raster(&raster, view, [64, 64]).pixels
            );
            assert_eq!(
                reference.render_raster_half(&raster, view, [64, 64]),
                tiled.render_raster_half(&raster, view, [64, 64])
            );
        }
        for hdr in [false, true] {
            let budget = rrrah_core::MemoryBudget::new(bytes + 16 + 64);
            let uploads = rrrah_core::MemoryBudget::new(1024 * 1024 + 64);
            let texture_owner = budget.try_reserve(64).unwrap();
            let queue_owner = uploads.try_reserve(64).unwrap();
            let mut renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&tiled.device, budget.clone())
            } else {
                rrrah_gpu::RasterRenderer::new_with_budget(
                    &tiled.device,
                    common::READBACK_FORMAT,
                    budget.clone(),
                )
            }
            .with_upload_queue_budget(uploads.clone());
            let read = |renderer: &rrrah_gpu::RasterRenderer| -> Vec<u16> {
                if hdr {
                    tiled.render_existing_raster_half(renderer, [64, 64])
                } else {
                    tiled
                        .render_existing_raster(renderer, [64, 64])
                        .pixels
                        .into_iter()
                        .map(u16::from)
                        .collect()
                }
            };
            let old = frame(1, vec![4., -0.5, 0., 1.]);
            renderer.upload(&tiled.device, &tiled.queue, &old).unwrap();
            renderer.update_view(&tiled.queue, view());
            let before = read(&renderer);
            assert_eq!((budget.used(), uploads.used()), (80, 64));
            // Measure every callback after resource admission, including each
            // tile boundary and the final check before installing the texture.
            let calls = std::cell::Cell::new(0usize);
            renderer
                .upload_with_cancel(&tiled.device, &tiled.queue, &raster, || {
                    if budget.used() > 80 {
                        calls.set(calls.get() + 1);
                    }
                    false
                })
                .unwrap();
            tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(calls.get(), (columns * rows + 2) as usize);
            renderer.upload(&tiled.device, &tiled.queue, &old).unwrap();
            tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(read(&renderer), before);
            for cutoff in 1..=calls.get() {
                let observed = std::cell::Cell::new(0usize);
                assert!(matches!(
                    renderer.upload_with_cancel(&tiled.device, &tiled.queue, &raster, || {
                        if budget.used() > 80 {
                            observed.set(observed.get() + 1);
                        }
                        observed.get() == cutoff
                    }),
                    Err(rrrah_gpu::RasterUploadError::Cancelled)
                ));
                tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert_eq!(observed.get(), cutoff);
                assert_eq!((budget.used(), uploads.used()), (80, 64));
                assert_eq!(read(&renderer), before);
            }
            renderer.upload(&tiled.device, &tiled.queue, &raster).unwrap();
            tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let expected: Vec<u16> = if hdr {
                reference.render_raster_half(&raster, view(), [64, 64])
            } else {
                reference
                    .render_raster(&raster, view(), [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            assert_eq!(read(&renderer), expected);
            assert_eq!(renderer.resident_bytes(), bytes);
            assert_eq!((budget.used(), uploads.used()), (bytes + 64, 64));
            drop(renderer);
            tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!((budget.used(), uploads.used()), (64, 64));
            drop(texture_owner);
            drop(queue_owner);
            assert_eq!((budget.used(), uploads.used()), (0, 0));
            eprintln!(
                "TILED_UPLOAD_CANCEL dimensions={width}x{height} hdr={hdr} admitted_cutoffs={} previous_frame_exact=true retry_exact=true unrelated_owners_preserved=true final=0",
                calls.get()
            );
        }
    }
}

#[test]
fn resident_compute_tiles_match_cpu_sdr_hdr_across_aligned_and_packed_rows() {
    let reference = common::qualification_gpu().expect("actual GPU required");
    let mut tiled = common::qualification_gpu().expect("actual GPU required");
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    tiled.device = device;
    tiled.queue = queue;
    for [width, height] in [[130u32, 5u32], [5, 130], [128, 130], [130, 130], [20000, 64]] {
        let gpu = if width > 1000 { &reference } else { &tiled };
        let count = width * height;
        let pixels: Vec<[f32; 4]> = (0..count)
            .map(|i| {
                [
                    (i % 17) as f32 / 8. - 0.5,
                    (i % 7) as f32 / 4.,
                    0.75,
                    [0., 0.25, 0.5, 1.][(i % 4) as usize],
                ]
            })
            .collect();
        let expected = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(
                Arc::new(
                    pixels
                        .iter()
                        .flat_map(|p| [p[0] * 2., p[1] * 2., p[2] * 2., p[3]])
                        .collect::<Vec<_>>(),
                )
                .into(),
            ),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let fit = (32. / width as f32).min(32. / height as f32);
        let zoom = (1. / fit).min(128.);
        let parameters = ViewParameters {
            viewport: [64., 64.],
            zoom,
            ..Default::default()
        };
        let a = reference.render_raster(&expected, parameters, [64, 64]);
        let b = gpu.render_resident_exposure(&pixels, [width, height], 1., parameters, [64, 64]);
        assert_eq!(a.pixels, b.pixels);
        assert!(
            a.pixels.chunks_exact(4).any(|p| p != &a.pixels[..4]),
            "nonconstant control required"
        );
        let bytes = u64::from(count) * 16;
        let compute_budget = rrrah_core::MemoryBudget::new(bytes * 2 + 16);
        let texture_bytes =
            rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&gpu.device, width, height).unwrap();
        let texture_budget = rrrah_core::MemoryBudget::new(texture_bytes);
        let queue_budget = rrrah_core::MemoryBudget::new(bytes);
        let compute = rrrah_gpu::LinearExposureCompute::new(&gpu.device);
        let resident = compute
            .execute_resident(&gpu.queue, &pixels, 1., &compute_budget, || false)
            .unwrap();
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone())
                .with_upload_queue_budget(queue_budget.clone());
        renderer
            .upload_resident(&gpu.device, &gpu.queue, &resident, [width, height], || false)
            .unwrap();
        drop(resident);
        renderer.update_view(&gpu.queue, parameters);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            reference.render_raster_half(&expected, parameters, [64, 64])
        );
        // Sample the cropped last layer and last source row/column, not just seams.
        let scale = fit * zoom;
        let edge = ViewParameters {
            pan: [
                (width as f32 / 2. - (width - 1) as f32) * scale + 0.5,
                (height as f32 / 2. - (height - 1) as f32) * scale + 0.5,
            ],
            ..parameters
        };
        renderer.update_view(&gpu.queue, edge);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            reference.render_raster_half(&expected, edge, [64, 64])
        );
        assert_eq!(compute_budget.used(), 0);
        assert_eq!(queue_budget.used(), 0);
        assert_eq!(texture_budget.used(), texture_bytes);
        drop(renderer);
        assert_eq!(texture_budget.used(), 0);
        eprintln!(
            "resident tiles: {width}x{height}; SDR/HDR exact; padded_bytes={texture_bytes}; compute/queue/texture root=0"
        );
    }
}

#[test]
fn resident_source_regions_preserve_full_geometry_hdr_and_transactional_admission() {
    let reference = common::qualification_gpu().expect("actual GPU required");
    let mut limited = common::qualification_gpu().expect("actual GPU required");
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    limited.device = device;
    limited.queue = queue;
    let source = DecodedRaster::new(
        200,
        160,
        RasterPixels::Rgba32Float(
            Arc::new(
                (0..200 * 160)
                    .flat_map(|i| {
                        [
                            (i % 29) as f32 / 8. - 0.5,
                            (i % 13) as f32 / 4.,
                            (i / 200) as f32 / 80.,
                            [0., 0.25, 0.75, 1.][(i % 4) as usize],
                        ]
                    })
                    .collect::<Vec<_>>(),
            )
            .into(),
        ),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    // Both a single layer and multiple cropped layers retain original image coordinates.
    for (gpu, rect) in [
        (&reference, [35, 40, 130, 70]),
        (&limited, [35, 40, 130, 70]),
        (&limited, [0, 0, 70, 70]),
        (&limited, [130, 90, 70, 70]),
    ] {
        let bytes =
            rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&gpu.device, rect[2], rect[3]).unwrap();
        assert!(bytes < 200 * 160 * 16);
        let root = rrrah_core::MemoryBudget::new(bytes * 2);
        let queued = rrrah_core::MemoryBudget::new(bytes * 2 + 4096);
        for hdr in [false, true] {
            let mut renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, root.clone())
            } else {
                rrrah_gpu::RasterRenderer::new_with_budget(
                    &gpu.device,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    root.clone(),
                )
            }
            .with_upload_queue_budget(queued.clone());
            renderer
                .upload_region_with_cancel(&gpu.device, &gpu.queue, &source, rect, || false)
                .unwrap();
            let center = [
                rect[0] as f32 + rect[2] as f32 / 2.,
                rect[1] as f32 + rect[3] as f32 / 2.,
            ];
            let parameters = ViewParameters {
                viewport: [64., 64.],
                zoom: 8.,
                pan: [(100. - center[0]) * 1.28, (80. - center[1]) * 1.28],
                ..Default::default()
            };
            renderer.update_view(&gpu.queue, parameters);
            let before = if hdr {
                gpu.render_existing_raster_half(&renderer, [64, 64])
            } else {
                gpu.render_existing_raster(&renderer, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            let expected = if hdr {
                reference.render_raster_half(&source, parameters, [64, 64])
            } else {
                reference
                    .render_raster(&source, parameters, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            assert_eq!(before, expected, "region={rect:?} hdr={hdr}");
            assert_eq!((root.used(), queued.used()), (bytes, 0));
            let pressure = root.try_reserve(root.available_bytes()).unwrap();
            assert!(matches!(
                renderer.upload_region_with_cancel(&gpu.device, &gpu.queue, &source, rect, || false),
                Err(rrrah_gpu::RasterUploadError::Memory(_))
            ));
            drop(pressure);
            for bad in [[0, 0, 0, 1], [199, 0, 2, 1], [0, u32::MAX, 1, 2]] {
                assert!(matches!(
                    renderer.upload_region_with_cancel(&gpu.device, &gpu.queue, &source, bad, || false),
                    Err(rrrah_gpu::RasterUploadError::InvalidRegion)
                ));
            }
            let mut after_admission = 0;
            assert!(matches!(
                renderer.upload_region_with_cancel(&gpu.device, &gpu.queue, &source, rect, || {
                    if queued.used() > 0 {
                        after_admission += 1;
                    }
                    after_admission >= 4
                }),
                Err(rrrah_gpu::RasterUploadError::Cancelled)
            ));
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!((root.used(), queued.used()), (bytes, 0));
            let after = if hdr {
                gpu.render_existing_raster_half(&renderer, [64, 64])
            } else {
                gpu.render_existing_raster(&renderer, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            assert_eq!(after, before);
            // A footprint outside residency must show background, never a clamped edge smear.
            let absent = if rect[0] == 0 { [190., 150.] } else { [10., 10.] };
            renderer.update_view(
                &gpu.queue,
                ViewParameters {
                    viewport: [64., 64.],
                    zoom: 128.,
                    pan: [(100. - absent[0]) * 20.48, (80. - absent[1]) * 20.48],
                    ..Default::default()
                },
            );
            let mut empty = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr(&gpu.device)
            } else {
                rrrah_gpu::RasterRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb)
            };
            // Compare with ordinary shader background. Metal attachment-clear sRGB
            // conversion can differ by one code from fragment-output conversion.
            empty
                .upload(&gpu.device, &gpu.queue, &frame(1, vec![1.; 4]))
                .unwrap();
            empty.update_view(
                &gpu.queue,
                ViewParameters {
                    viewport: [64., 64.],
                    pan: [10000., 10000.],
                    ..Default::default()
                },
            );
            if hdr {
                assert_eq!(
                    gpu.render_existing_raster_half(&renderer, [64, 64]),
                    gpu.render_existing_raster_half(&empty, [64, 64])
                );
            } else {
                assert_eq!(
                    gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                    gpu.render_existing_raster(&empty, [64, 64]).pixels
                );
            }
            renderer
                .upload_region_with_cancel(&gpu.device, &gpu.queue, &source, rect, || false)
                .unwrap();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(root.used(), bytes);
            // Ordinary whole-image upload resets region origin and selects the ordinary pipeline.
            let tiny = frame(1, vec![2., 0.5, 0., 1.]);
            renderer.upload(&gpu.device, &gpu.queue, &tiny).unwrap();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(root.used(), 16);
            drop(renderer);
            assert_eq!((root.used(), queued.used()), (0, 0));
        }
        eprintln!(
            "resident source region={rect:?}; bytes={bytes}; full geometry SDR/HDR exact; root/queue=0"
        );
    }
}

#[test]
fn small_resident_region_renders_source_above_eager_atlas_limit() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let color = [-0.5, 2., 0.25, 0.75];
    let source = DecodedRaster::new(
        6000,
        6000,
        RasterPixels::Rgba32Float(Arc::new(color.repeat(6000 * 6000)).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let bytes = 128 * 128 * 16;
    let root = rrrah_core::MemoryBudget::new(bytes);
    let queued = rrrah_core::MemoryBudget::new(bytes);
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, root.clone())
        .with_upload_queue_budget(queued.clone());
    assert!(matches!(
        renderer.upload(&gpu.device, &gpu.queue, &source),
        Err(rrrah_gpu::RasterUploadError::TextureLimit)
    ));
    assert_eq!((root.used(), queued.used()), (0, 0));
    renderer
        .upload_region_with_cancel(&gpu.device, &gpu.queue, &source, [2936, 2936, 128, 128], || false)
        .unwrap();
    renderer.update_view(
        &gpu.queue,
        ViewParameters {
            viewport: [64., 64.],
            zoom: 128.,
            ..Default::default()
        },
    );
    let actual = gpu.render_existing_raster_half(&renderer, [64, 64]);
    let reference = frame(1, color.to_vec());
    let expected = gpu.render_raster_half(
        &reference,
        ViewParameters {
            viewport: [64., 64.],
            zoom: 4.,
            ..Default::default()
        },
        [64, 64],
    );
    assert_eq!(actual, expected);
    assert_eq!((root.used(), queued.used()), (bytes, 0));
    drop(renderer);
    assert_eq!(root.used(), 0);
    eprintln!(
        "6000x6000 source=576000000 bytes > eager limit; resident 128x128=262144 bytes; full HDR viewport exact; root/queue=0"
    );
}

#[test]
fn planned_visible_regions_match_whole_image_during_pan_zoom_and_pixel_aspect() {
    let reference = common::qualification_gpu().expect("actual GPU required");
    let mut tiled = common::qualification_gpu().expect("actual GPU required");
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    tiled.device = device;
    tiled.queue = queue;
    let pixels = Arc::new(
        (0..512 * 320)
            .flat_map(|i| {
                [
                    (i % 37) as f32 / 8. - 0.5,
                    (i / 512) as f32 / 80.,
                    (i % 11) as f32 / 4.,
                    [0., 0.25, 0.75, 1.][(i % 4) as usize],
                ]
            })
            .collect::<Vec<_>>(),
    );
    let mut checked = 0;
    let mut skipped = 0;
    for aspect in [0.5, 1., 2.] {
        let raster = DecodedRaster::new(
            512,
            320,
            RasterPixels::Rgba32Float(pixels.clone().into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(aspect))
        .unwrap();
        let full = rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&tiled.device, 512, 320).unwrap();
        for hdr in [false, true] {
            let root = rrrah_core::MemoryBudget::new(full * 2);
            let queued = rrrah_core::MemoryBudget::new(full * 4);
            let mut renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&tiled.device, root.clone())
            } else {
                rrrah_gpu::RasterRenderer::new_with_budget(
                    &tiled.device,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    root.clone(),
                )
            }
            .with_upload_queue_budget(queued.clone());
            for (zoom, pan) in [
                (1., [0., 0.]),
                (8., [0., 0.]),
                (128., [0., 0.]),
                (8., [43.25, -17.75]),
                (4., [-87.5, 19.25]),
                (2., [20., -30.]),
                (1., [10000., 0.]),
                (8., [0., -10000.]),
                (0.02, [0., 0.]),
                (1., [0., 0.]),
            ] {
                let view = ViewParameters {
                    viewport: [64., 64.],
                    zoom,
                    pan,
                    exposure_stops: 1.25,
                };
                let rect = rrrah_gpu::RasterRenderer::visible_source_region(&raster, view).unwrap();
                let expected_bytes = if let Some(rect) = rect {
                    assert!(
                        rect[2] > 0 && rect[3] > 0 && rect[0] + rect[2] <= 512 && rect[1] + rect[3] <= 320
                    );
                    renderer
                        .upload_region_with_cancel(&tiled.device, &tiled.queue, &raster, rect, || false)
                        .unwrap();
                    checked += 1;
                    rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&tiled.device, rect[2], rect[3])
                        .unwrap()
                } else {
                    skipped += 1;
                    renderer.resident_bytes()
                };
                renderer.update_view(&tiled.queue, view);
                if hdr {
                    let actual = tiled.render_existing_raster_half(&renderer, [64, 64]);
                    let expected = reference.render_raster_half(&raster, view, [64, 64]);
                    assert_eq!(
                        actual, expected,
                        "aspect={aspect} zoom={zoom} pan={pan:?} rect={rect:?}"
                    );
                } else {
                    let actual = tiled.render_existing_raster(&renderer, [64, 64]);
                    let expected = reference.render_raster(&raster, view, [64, 64]);
                    assert_eq!(
                        actual.pixels, expected.pixels,
                        "aspect={aspect} zoom={zoom} pan={pan:?} rect={rect:?}"
                    );
                }
                assert_eq!((root.used(), queued.used()), (expected_bytes, 0));
            }
            drop(renderer);
            assert_eq!((root.used(), queued.used()), (0, 0));
        }
    }
    assert_eq!((checked, skipped), (46, 14));
    eprintln!(
        "visible-region planner: 60 exact full SDR/HDR view frames; uploads={checked}; offscreen_skips={skipped}; aspects=0.5/1/2; root/queue=0"
    );
}

#[test]
fn area_filtered_hdr_lod_matches_independent_alpha_reference_on_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let pixels: Vec<f32> = (0..128 * 64)
        .flat_map(|i| {
            let x = i % 128;
            let y = i / 128;
            match (x % 2, y % 2) {
                (0, 0) => [-2., 8., 0.5, 1.],
                (1, 0) => [-2., 8., 0.5, 0.5],
                _ => [1000., -1000., 500., 0.],
            }
        })
        .collect();
    let source = DecodedRaster::new(
        128,
        64,
        RasterPixels::Rgba32Float(Arc::new(pixels).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let budget = rrrah_core::MemoryBudget::new(16 * 8 * 16);
    let lod = source
        .downsample_linear_with_budget_and_cancel([16, 8], &budget, || false)
        .unwrap();
    let expected = DecodedRaster::new(
        16,
        8,
        RasterPixels::Rgba32Float(Arc::new([-2., 8., 0.5, 0.375].repeat(16 * 8)).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let params = ViewParameters {
        viewport: [64., 64.],
        zoom: 1.25,
        exposure_stops: 0.75,
        ..Default::default()
    };
    assert_eq!(
        gpu.render_raster(&lod, params, [64, 64]).pixels,
        gpu.render_raster(&expected, params, [64, 64]).pixels
    );
    assert_eq!(
        gpu.render_raster_half(&lod, params, [64, 64]),
        gpu.render_raster_half(&expected, params, [64, 64])
    );
    assert_eq!(budget.used(), 2048);
    drop(lod);
    assert_eq!(budget.used(), 0);
    eprintln!("managed area HDR LOD: signed/bright/transparent-RGB oracle SDR/HDR exact; output root=0");
}

fn qualify_large_lod_pressure(source: &DecodedRaster, lod: DecodedRaster, budget: &rrrah_core::MemoryBudget) {
    assert_eq!(budget.used(), 16000000);
    let held = lod.clone();
    drop(lod);
    assert_eq!(budget.used(), 16000000);
    assert!(matches!(
        source.downsample_linear_with_budget_and_cancel([1000, 1000], budget, || false),
        Err(rrrah_core::RasterError::Memory(_))
    ));
    drop(held);
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(1).unwrap();
    assert!(matches!(
        source.downsample_linear_with_budget_and_cancel([1000, 1000], budget, || false),
        Err(rrrah_core::RasterError::Memory(_))
    ));
    assert_eq!(budget.used(), 1);
    drop(pressure);
    let mut checks = 0;
    let mut admitted = false;
    assert!(matches!(
        source.downsample_linear_with_budget_and_cancel([1000, 1000], budget, || {
            checks += 1;
            admitted |= budget.used() == 16000000;
            checks == 5
        }),
        Err(rrrah_core::RasterError::Cancelled)
    ));
    assert!(admitted, "cancellation must observe admitted output");
    assert_eq!(budget.used(), 0);
    let retry = source
        .downsample_linear_with_budget_and_cancel([1000, 1000], budget, || false)
        .unwrap();
    let RasterPixels::Rgba32Float(pixels) = retry.pixels() else {
        panic!()
    };
    for pixel in pixels.chunks_exact(4) {
        assert_eq!(pixel, [-2., 8., 0.5, 0.375]);
    }
    drop(retry);
    assert_eq!(budget.used(), 0);
    eprintln!(
        "36 MP HDR LOD pressure: held clone refuses replacement; one-byte pressure refuses; admitted cancellation releases; retry all1000000 exact; final0"
    );
}

#[test]
#[ignore = "manual 36 MP HDR area LOD and actual Metal qualification"]
fn large_hdr_area_lod_preserves_all_output_samples_and_metal_frame() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("36 MP HDR LOD adapter: {}", gpu.adapter_name());
    let mut samples = Vec::with_capacity(6000 * 6000 * 4);
    for y in 0..6000 {
        for x in 0..6000 {
            samples.extend_from_slice(&match (x % 2, y % 2) {
                (0, 0) => [-2., 8., 0.5, 1.],
                (1, 0) => [-2., 8., 0.5, 0.5],
                _ => [1000., -1000., 500., 0.],
            });
        }
    }
    let source = DecodedRaster::new(
        6000,
        6000,
        RasterPixels::Rgba32Float(Arc::new(samples).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let budget = rrrah_core::MemoryBudget::new(1000 * 1000 * 16);
    let started = std::time::Instant::now();
    let lod = source
        .downsample_linear_with_budget_and_cancel([1000, 1000], &budget, || false)
        .unwrap();
    let elapsed = started.elapsed();
    let RasterPixels::Rgba32Float(output) = lod.pixels() else {
        panic!()
    };
    for pixel in output.chunks_exact(4) {
        assert_eq!(pixel, [-2., 8., 0.5, 0.375]);
    }
    let reference = frame(1, vec![-2., 8., 0.5, 0.375]);
    for output_size in [[64, 64], [1000, 1000]] {
        let view = ViewParameters {
            viewport: [output_size[0] as f32, output_size[1] as f32],
            zoom: 4.,
            exposure_stops: 0.75,
            ..Default::default()
        };
        assert_eq!(
            gpu.render_raster(&lod, view, output_size).pixels,
            gpu.render_raster(&reference, view, output_size).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&lod, view, output_size),
            gpu.render_raster_half(&reference, view, output_size)
        );
        eprintln!(
            "36 MP HDR area LOD full output: {}x{}; every SDR/HDR component matches independent constant reference",
            output_size[0], output_size[1]
        );
    }
    qualify_large_lod_pressure(&source, lod, &budget);
    eprintln!(
        "36 MP HDR area LOD: source=576000000; output=16000000; all1000000 pixels exact; SDR/HDR Metal exact; elapsed={elapsed:?}; output root=0"
    );
}

#[test]
fn nonfinite_raster_controls_produce_exact_finite_sdr_hdr_frames() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let source = frame(2, vec![8., -2., 1., 1., 0.25, 2., 0., 1.]);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for invalid_viewport in [false, true] {
            let viewport = if invalid_viewport { [1., 1.] } else { [64., 64.] };
            let expected = ViewParameters {
                viewport,
                ..Default::default()
            };
            let actual = ViewParameters {
                viewport: if invalid_viewport { [invalid; 2] } else { viewport },
                pan: [invalid; 2],
                zoom: invalid,
                exposure_stops: invalid,
            };
            assert_eq!(
                gpu.render_raster(&source, actual, [64, 64]).pixels,
                gpu.render_raster(&source, expected, [64, 64]).pixels
            );
            let hdr = gpu.render_raster_half(&source, actual, [64, 64]);
            assert!(hdr.iter().all(|bits| bits & 0x7c00 != 0x7c00));
            assert_eq!(hdr, gpu.render_raster_half(&source, expected, [64, 64]));
        }
    }
}

#[test]
fn calibrated_bayer_resident_upload_preserves_frames_budgets_sdr_hdr() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let mosaic = common::profiled_pattern_mosaic(
        17,
        13,
        1000.,
        [2., 1., 0.5, 1.],
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
        |x, y| (100 + (x * 43 + y * 29) % 2000) as u16,
    );
    let texture_bytes = rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&gpu.device, 17, 13).unwrap();
    let parameters = ViewParameters {
        viewport: [64., 64.],
        zoom: 2.,
        ..Default::default()
    };
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&gpu.device);
    for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
        let compute_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu_budget = rrrah_core::MemoryBudget::new(17 * 13 * 4);
        let resident = compute
            .execute_mosaic_resident(
                &gpu.queue,
                &mosaic,
                algorithm,
                &compute_budget,
                &cpu_budget,
                || false,
            )
            .unwrap();
        let bytes = 17 * 13 * 16;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bayer upload oracle pixels"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(resident.buffer(), 0, &readback, 0, bytes);
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
            tx.send(status).unwrap();
        });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let expected = DecodedRaster::new(
            17,
            13,
            RasterPixels::Rgba32Float(Arc::new(bytemuck::cast_slice::<u8, f32>(&mapped).to_vec()).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        drop(mapped);
        readback.unmap();
        for hdr in [false, true] {
            let texture_budget = rrrah_core::MemoryBudget::new(texture_bytes * 2);
            let queue_budget = rrrah_core::MemoryBudget::new(bytes);
            let mut renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone())
            } else {
                rrrah_gpu::RasterRenderer::new_with_budget(
                    &gpu.device,
                    common::READBACK_FORMAT,
                    texture_budget.clone(),
                )
            }
            .with_upload_queue_budget(queue_budget.clone());
            renderer
                .upload(&gpu.device, &gpu.queue, &frame(1, vec![0.18, 0.18, 0.18, 1.]))
                .unwrap();
            renderer.update_view(&gpu.queue, parameters);
            let old = if hdr {
                gpu.render_existing_raster_half(&renderer, [64, 64])
            } else {
                gpu.render_existing_raster(&renderer, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            let pressure = texture_budget
                .try_reserve(texture_budget.available_bytes())
                .unwrap();
            assert!(
                renderer
                    .upload_bayer_resident(&gpu.device, &gpu.queue, &resident, || false)
                    .is_err()
            );
            drop(pressure);
            let mut checks = 0;
            assert!(matches!(
                renderer.upload_bayer_resident(&gpu.device, &gpu.queue, &resident, || {
                    checks += 1;
                    checks == 2
                }),
                Err(rrrah_gpu::RasterUploadError::Cancelled)
            ));
            let preserved = if hdr {
                gpu.render_existing_raster_half(&renderer, [64, 64])
            } else {
                gpu.render_existing_raster(&renderer, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            assert_eq!(old, preserved);
            renderer
                .upload_bayer_resident(&gpu.device, &gpu.queue, &resident, || false)
                .unwrap();
            renderer.update_view(&gpu.queue, parameters);
            let actual = if hdr {
                gpu.render_existing_raster_half(&renderer, [64, 64])
            } else {
                gpu.render_existing_raster(&renderer, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            let reference = if hdr {
                gpu.render_raster_half(&expected, parameters, [64, 64])
            } else {
                gpu.render_raster(&expected, parameters, [64, 64])
                    .pixels
                    .into_iter()
                    .map(u16::from)
                    .collect()
            };
            assert_eq!(actual, reference, "{algorithm:?} hdr={hdr}");
            assert_eq!(queue_budget.used(), 0);
            assert_eq!(texture_budget.used(), texture_bytes);
            drop(renderer);
            assert_eq!(texture_budget.used(), 0);
        }
        let texture_budget = rrrah_core::MemoryBudget::new(texture_bytes);
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone());
        renderer
            .upload_bayer_resident(&gpu.device, &gpu.queue, &resident, || false)
            .unwrap();
        drop(resident);
        // The queued GPU copy, rather than the producer, now owns this credit.
        assert!(compute_budget.used() > 0);
        renderer.update_view(&gpu.queue, parameters);
        assert_eq!(
            gpu.render_existing_raster_half(&renderer, [64, 64]),
            gpu.render_raster_half(&expected, parameters, [64, 64])
        );
        assert_eq!((compute_budget.used(), cpu_budget.used()), (0, 0));
        drop(renderer);
        assert_eq!(texture_budget.used(), 0);
        eprintln!(
            "calibrated Bayer upload {algorithm:?}: SDR/HDR whole64x64 exact, pressure/cancel preserve prior frame, credits zero"
        );
    }
}

#[test]
fn bayer_resident_crop_and_all_orientations_match_materialized_sdr_hdr() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    check_bayer_resident_geometry(&gpu, 17, 13, rrrah_core::Rect::new(3, 2, 11, 7));
}

#[test]
fn tiled_bayer_resident_crop_and_all_orientations_match_materialized_sdr_hdr() {
    let mut gpu = common::qualification_gpu().expect("actual GPU required");
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 64,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    gpu.device = device;
    gpu.queue = queue;
    check_bayer_resident_geometry(&gpu, 130, 129, rrrah_core::Rect::new(23, 17, 93, 87));
}

fn check_bayer_resident_geometry(
    gpu: &common::GpuReadback,
    sensor_width: u32,
    sensor_height: u32,
    crop: rrrah_core::Rect,
) {
    use rrrah_core::Orientation::*;
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&gpu.device);
    for orientation in [
        Normal,
        HorizontalFlip,
        Rotate180,
        VerticalFlip,
        Transpose,
        Rotate90,
        Transverse,
        Rotate270,
    ] {
        let mut mosaic = common::profiled_pattern_mosaic(
            sensor_width,
            sensor_height,
            1000.,
            [2., 1., 0.5, 1.],
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
            |x, y| (100 + (x * 43 + y * 29) % 2000) as u16,
        );
        mosaic.metadata.crop_area = Some(crop);
        mosaic.metadata.orientation = orientation;
        let compute_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu_budget =
            rrrah_core::MemoryBudget::new(u64::from(sensor_width) * u64::from(sensor_height) * 4);
        let resident = compute
            .execute_mosaic_resident(
                &gpu.queue,
                &mosaic,
                rrrah_gpu::GpuDemosaicAlgorithm::Bilinear,
                &compute_budget,
                &cpu_budget,
                || false,
            )
            .unwrap();
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bayer geometry pixel oracle"),
            size: u64::from(sensor_width) * u64::from(sensor_height) * 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(
            resident.buffer(),
            0,
            &readback,
            0,
            u64::from(sensor_width) * u64::from(sensor_height) * 16,
        );
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
            tx.send(status).unwrap();
        });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let source = bytemuck::cast_slice::<u8, [f32; 4]>(&mapped);
        let [width, height] = if orientation.swaps_dimensions() {
            [crop.height, crop.width]
        } else {
            [crop.width, crop.height]
        };
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let uv = orientation.map_display_uv_f64([
                    (f64::from(x) + 0.5) / f64::from(width),
                    (f64::from(y) + 0.5) / f64::from(height),
                ]);
                let sx = crop.x + (uv[0] * f64::from(crop.width)).floor() as u32;
                let sy = crop.y + (uv[1] * f64::from(crop.height)).floor() as u32;
                pixels.extend_from_slice(&source[(sy * sensor_width + sx) as usize]);
            }
        }
        let expected = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        drop(mapped);
        readback.unmap();
        let parameters = ViewParameters {
            viewport: [64., 64.],
            zoom: 1.25,
            pan: [3., -2.],
            exposure_stops: 0.5,
        };
        for hdr in [false, true] {
            let budget = rrrah_core::MemoryBudget::new(
                rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&gpu.device, sensor_width, sensor_height)
                    .unwrap()
                    * 2,
            );
            let mut renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone())
            } else {
                rrrah_gpu::RasterRenderer::new_with_budget(
                    &gpu.device,
                    common::READBACK_FORMAT,
                    budget.clone(),
                )
            };
            renderer
                .upload_bayer_resident(&gpu.device, &gpu.queue, &resident, || false)
                .unwrap();
            renderer.update_view(&gpu.queue, parameters);
            let mut reference_renderer = if hdr {
                rrrah_gpu::RasterRenderer::new_linear_hdr(&gpu.device)
            } else {
                rrrah_gpu::RasterRenderer::new(&gpu.device, common::READBACK_FORMAT)
            };
            reference_renderer
                .upload(&gpu.device, &gpu.queue, &expected)
                .unwrap();
            reference_renderer.set_area_minification(&gpu.queue, true);
            reference_renderer.update_view(&gpu.queue, parameters);
            if hdr {
                assert_eq!(
                    gpu.render_existing_raster_half(&renderer, [64, 64]),
                    gpu.render_existing_raster_half(&reference_renderer, [64, 64]),
                    "{orientation:?}"
                );
            } else {
                assert_eq!(
                    gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                    gpu.render_existing_raster(&reference_renderer, [64, 64]).pixels,
                    "{orientation:?}"
                );
            }
            let ordinary = frame(1, vec![0.18, 0.27, 0.36, 1.]);
            renderer.upload(&gpu.device, &gpu.queue, &ordinary).unwrap();
            renderer.update_view(&gpu.queue, parameters);
            if hdr {
                assert_eq!(
                    gpu.render_existing_raster_half(&renderer, [64, 64]),
                    gpu.render_raster_half(&ordinary, parameters, [64, 64])
                );
            } else {
                assert_eq!(
                    gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                    gpu.render_raster(&ordinary, parameters, [64, 64]).pixels
                );
            }
            drop(renderer);
            assert_eq!(budget.used(), 0);
        }
        drop(resident);
        assert_eq!((compute_budget.used(), cpu_budget.used()), (0, 0));
        eprintln!(
            "Bayer geometry {orientation:?}: offset crop, whole SDR/HDR exact, ordinary upload resets geometry, credits zero"
        );
    }
}

#[test]
fn calibrated_bayer_hdr_exposure_has_explicit_signed_half_range() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let mosaic = common::profiled_pattern_mosaic(
        16,
        16,
        1.,
        [1.; 4],
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
        |x, y| if (x + y) % 2 == 1 { 4000 } else { 1000 },
    );
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&gpu.device);
    for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(16 * 16 * 4);
        let output = compute
            .execute_mosaic_resident(&gpu.queue, &mosaic, algorithm, &root, &cpu, || false)
            .unwrap();
        let texture_budget = rrrah_core::MemoryBudget::new(16 * 16 * 16);
        let mut renderer =
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, texture_budget.clone());
        renderer
            .upload_bayer_resident(&gpu.device, &gpu.queue, &output, || false)
            .unwrap();
        drop(output);
        renderer.update_view(
            &gpu.queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 2.,
                exposure_stops: 10.,
                ..Default::default()
            },
        );
        let pixels = gpu.render_existing_raster_half(&renderer, [64, 64]);
        for pixel in pixels.chunks_exact(4) {
            assert_eq!(pixel, [0xfbff, 0x7bff, 0x7bff, 0x3c00], "{algorithm:?}");
        }
        assert_eq!((root.used(), cpu.used()), (0, 0));
        drop(renderer);
        assert_eq!(texture_budget.used(), 0);
        eprintln!(
            "Bayer HDR range {algorithm:?}: signed +/-65504 after10stops, all4096 pixels exact, alpha1, credits zero"
        );
    }
}

#[test]
fn calibrated_bayer_compute_and_fragment_respect_shader_precision_at_sensor_pixel_scale() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let mosaic = common::profiled_pattern_mosaic(
        64,
        64,
        1024.,
        [2., 1., 0.5, 1.],
        [inverse[0], inverse[1], inverse[2], [0.; 3]],
        |x, y| (100 + (x * 43 + y * 29) % 2000) as u16,
    );
    let parameters = ViewParameters {
        viewport: [64., 64.],
        zoom: 2.,
        ..Default::default()
    };
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&gpu.device);
    for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
        let mut raw = rrrah_gpu::RawRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba16Float);
        raw.set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
            .unwrap();
        raw.set_demosaic_algorithm(&gpu.queue, algorithm);
        raw.upload_mosaic(&gpu.device, &gpu.queue, &mosaic).unwrap();
        raw.update_view(&gpu.queue, parameters);
        let reference = gpu.render_existing_raw_half(&raw, [64, 64]);
        let gpu_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu_budget = rrrah_core::MemoryBudget::new(64 * 64 * 4);
        let output = compute
            .execute_mosaic_resident(&gpu.queue, &mosaic, algorithm, &gpu_budget, &cpu_budget, || false)
            .unwrap();
        let mut raster = rrrah_gpu::RasterRenderer::new_linear_hdr(&gpu.device);
        raster
            .upload_bayer_resident(&gpu.device, &gpu.queue, &output, || false)
            .unwrap();
        drop(output);
        raster.update_view(&gpu.queue, parameters);
        let actual = gpu.render_existing_raster_half(&raster, [64, 64]);
        let mismatches: Vec<_> = actual
            .iter()
            .zip(&reference)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(8)
            .collect();
        let count = actual.iter().zip(&reference).filter(|(a, b)| a != b).count();
        let max_steps = actual
            .iter()
            .zip(&reference)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        eprintln!(
            "Bayer cross-stage {algorithm:?}: mismatched_channels={count} max_half_steps={max_steps} first={mismatches:?}"
        );
        // Dyadic fixture: simple filters match exactly. Weighted reciprocals may
        // straddle adjacent half quantization bins (WGSL15.7 allows reassociation/fusion).
        // A separate independent f64 oracle enforces f32 accuracy for every weighted pixel.
        if matches!(
            algorithm,
            rrrah_gpu::GpuDemosaicAlgorithm::Bilinear
                | rrrah_gpu::GpuDemosaicAlgorithm::SmoothHue
                | rrrah_gpu::GpuDemosaicAlgorithm::MalvarHeCutler
        ) {
            assert_eq!(count, 0, "{algorithm:?}");
        } else {
            assert!(
                max_steps <= 1,
                "{algorithm:?}: half quantization mismatch exceeds one step"
            );
        }
        for (&a, &b) in actual.iter().zip(&reference) {
            assert_ne!(a & 0x7c00, 0x7c00);
            assert_ne!(b & 0x7c00, 0x7c00);
        }
        assert_eq!((gpu_budget.used(), cpu_budget.used()), (0, 0));
        eprintln!(
            "Bayer fragment/compute {algorithm:?}: whole64x64 HDR compared at1:1, native normalization, credits zero"
        );
    }
}

#[path = "common/bayer_directional_reference.rs"]
mod directional_reference;

#[test]
fn weighted_fragment_compute_float32_accuracy_against_independent_f64() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let mosaic = common::profiled_pattern_mosaic(
        64,
        64,
        1024.,
        [2., 1., 0.5, 1.],
        [inverse[0], inverse[1], inverse[2], [0.; 3]],
        |x, y| (100 + (x * 43 + y * 29) % 2000) as u16,
    );
    let parameters = ViewParameters {
        viewport: [64., 64.],
        zoom: 2.,
        ..Default::default()
    };
    for algorithm in [
        rrrah_gpu::GpuDemosaicAlgorithm::Directional,
        rrrah_gpu::GpuDemosaicAlgorithm::Vng,
        rrrah_gpu::GpuDemosaicAlgorithm::Rcd,
    ] {
        let mut raw = rrrah_gpu::RawRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba32Float);
        raw.set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
            .unwrap();
        raw.set_demosaic_algorithm(&gpu.queue, algorithm);
        raw.upload_mosaic(&gpu.device, &gpu.queue, &mosaic).unwrap();
        raw.update_view(&gpu.queue, parameters);
        let fragment = gpu.read_float_frame([64, 64], |encoder, target| raw.encode(encoder, target));
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(64 * 64 * 4);
        let output = rrrah_gpu::BayerDemosaicCompute::new(&gpu.device)
            .execute_mosaic_resident(&gpu.queue, &mosaic, algorithm, &root, &cpu, || false)
            .unwrap();
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("independent directional f32 compute audit"),
            size: 64 * 64 * 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(output.buffer(), 0, &readback, 0, 64 * 64 * 16);
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
            tx.send(status).unwrap();
        });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let compute = bytemuck::cast_slice::<u8, f32>(&mapped).to_vec();
        drop(mapped);
        readback.unmap();
        drop(output);
        let mut maxima = [0.0f64; 3];
        for y in 0..64 {
            for x in 0..64 {
                let expected = directional_reference::weighted_rgb(&mosaic, x, y, algorithm);
                for channel in 0..3 {
                    let index = ((y * 64 + x) * 4 + channel as u32) as usize;
                    let a = f64::from(fragment[index]);
                    let b = f64::from(compute[index]);
                    maxima[0] = maxima[0].max((a - expected[channel]).abs());
                    maxima[1] = maxima[1].max((b - expected[channel]).abs());
                    maxima[2] = maxima[2].max((a - b).abs());
                    assert!(a.is_finite() && b.is_finite());
                    // This authored dyadic field isolates reciprocal/filter rounding.
                    // 32 f32 eps covers the composed weighted arithmetic, well below
                    // binary16 quantization here; this is not a bound for arbitrary inputs.
                    let tolerance = 32.0 * f64::from(f32::EPSILON) * expected[channel].abs().max(1.0);
                    assert!(
                        (a - expected[channel]).abs() <= tolerance,
                        "fragment {algorithm:?} {x},{y} channel={channel}"
                    );
                    assert!(
                        (b - expected[channel]).abs() <= tolerance,
                        "compute {algorithm:?} {x},{y} channel={channel}"
                    );
                }
            }
        }
        // Retain measured errors alongside the independent numerical accuracy gate.
        eprintln!(
            "{algorithm:?} independent f64 audit: max_abs_fragment={} max_abs_compute={} max_abs_cross_stage={}",
            maxima[0], maxima[1], maxima[2]
        );
        assert_eq!((root.used(), cpu.used()), (0, 0));
    }
}

#[test]
fn area_minification_matches_independent_sixteen_cell_linear_average() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    // Source is deliberately above1 and signed, with a high-frequency term.
    let source: Vec<[f32; 4]> = (0..64 * 64)
        .map(|i| {
            let x = i % 64;
            let y = i / 64;
            let high = if x % 4 == 0 && y % 4 == 0 { 32.0 } else { 0.0 };
            [
                high + x as f32 / 64.,
                -high - y as f32 / 128.,
                (x % 7) as f32 / 8.,
                1.,
            ]
        })
        .collect();
    let mut averaged = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            for channel in 0..4 {
                let mut sum = 0.0f64;
                for dy in 0..4 {
                    for dx in 0..4 {
                        sum += f64::from(source[((y * 4 + dy) * 64 + x * 4 + dx) as usize][channel]);
                    }
                }
                averaged.push((sum / 16.) as f32);
            }
        }
    }
    let full = DecodedRaster::new(
        64,
        64,
        RasterPixels::Rgba32Float(Arc::new(source.into_iter().flatten().collect::<Vec<_>>()).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let expected = DecodedRaster::new(
        16,
        16,
        RasterPixels::Rgba32Float(Arc::new(averaged).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    for hdr in [false, true] {
        let budget = rrrah_core::MemoryBudget::new(64 * 64 * 16);
        let mut renderer = if hdr {
            rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone())
        } else {
            rrrah_gpu::RasterRenderer::new_with_budget(&gpu.device, common::READBACK_FORMAT, budget.clone())
        };
        renderer.upload(&gpu.device, &gpu.queue, &full).unwrap();
        renderer.set_area_minification(&gpu.queue, true);
        renderer.update_view(
            &gpu.queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 0.5,
                ..Default::default()
            },
        );
        let parameters = ViewParameters {
            viewport: [64., 64.],
            zoom: 0.5,
            ..Default::default()
        };
        if hdr {
            assert_eq!(
                gpu.render_existing_raster_half(&renderer, [64, 64]),
                gpu.render_raster_half(&expected, parameters, [64, 64])
            );
        } else {
            assert_eq!(
                gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                gpu.render_raster(&expected, parameters, [64, 64]).pixels
            );
        }
        // Averaging takes place in linear space before RAW tone mapping.
        renderer.set_raw_development(&gpu.queue, &rrrah_core::develop::MonotoneCurve::identity());
        let mut reference = if hdr {
            rrrah_gpu::RasterRenderer::new_linear_hdr(&gpu.device)
        } else {
            rrrah_gpu::RasterRenderer::new(&gpu.device, common::READBACK_FORMAT)
        };
        reference.upload(&gpu.device, &gpu.queue, &expected).unwrap();
        reference.update_view(&gpu.queue, parameters);
        reference.set_raw_development(&gpu.queue, &rrrah_core::develop::MonotoneCurve::identity());
        if hdr {
            assert_eq!(
                gpu.render_existing_raster_half(&renderer, [64, 64]),
                gpu.render_existing_raster_half(&reference, [64, 64])
            );
        } else {
            assert_eq!(
                gpu.render_existing_raster(&renderer, [64, 64]).pixels,
                gpu.render_existing_raster(&reference, [64, 64]).pixels
            );
        }
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
    eprintln!(
        "area minification:64x64→16x16, independent f64 sixteen-cell average, HDR signed/bright and SDR exact, filtering before tone, credits zero"
    );
}

#[test]
fn area_source_region_missing_footprint_stays_unexposed_background() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let mut background = rrrah_gpu::RasterRenderer::new_linear_hdr(&gpu.device);
    background
        .upload(&gpu.device, &gpu.queue, &frame(1, vec![0., 0., 0., 1.]))
        .unwrap();
    background.update_view(
        &gpu.queue,
        ViewParameters {
            viewport: [64., 64.],
            pan: [1000., 1000.],
            ..Default::default()
        },
    );
    // Match shader background conversion, not the backend's render-pass clear conversion.
    let blank = gpu.render_existing_raster_half(&background, [64, 64]);
    for (edge, origin, visible) in [(3u32, [32, 32], 0usize), (8, [28, 28], 4)] {
        let crop = DecodedRaster::new(
            edge,
            edge,
            RasterPixels::Rgba32Float(Arc::new([4., 1., 0.5, 1.].repeat((edge * edge) as usize)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let budget = rrrah_core::MemoryBudget::new(u64::from(edge * edge) * 16);
        let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone());
        renderer
            .upload_packed_region_with_cancel(&gpu.device, &gpu.queue, &crop, [64, 64], origin, || false)
            .unwrap();
        renderer.set_area_minification(&gpu.queue, true);
        renderer.set_raw_development(&gpu.queue, &rrrah_core::develop::MonotoneCurve::identity());
        renderer.update_view(
            &gpu.queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 0.5,
                exposure_stops: 10.,
                ..Default::default()
            },
        );
        let actual = gpu.render_existing_raster_half(&renderer, [64, 64]);
        let displayed = actual
            .chunks_exact(4)
            .filter(|pixel| *pixel != &blank[..4])
            .count();
        assert_eq!(displayed, visible, "region edge={edge}");
        if visible == 0 {
            assert_eq!(actual, blank);
        }
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
    eprintln!(
        "area region: unavailable four-cell footprint returns untouched background even at+10stops/tone; complete footprints show4 pixels; credits zero"
    );
}

#[test]
fn fractional_area_alpha_matches_independent_f64_and_ignores_invisible_rgb() {
    fn half_value(word: u16) -> f64 {
        let sign = if word & 0x8000 != 0 { -1. } else { 1. };
        let exponent = (word >> 10) & 31;
        let fraction = word & 1023;
        assert_ne!(exponent, 31);
        sign * if exponent == 0 {
            f64::from(fraction) * 2.0f64.powi(-24)
        } else {
            f64::from(1024 + fraction) * 2.0f64.powi(i32::from(exponent) - 25)
        }
    }
    fn expected(source: &[[f32; 4]], size: [u32; 2], view: ViewParameters) -> Vec<[f64; 3]> {
        let [w, h] = size;
        let background = f64::from(0.018f32);
        let fit = (32.0 / f64::from(w)).min(32.0 / f64::from(h));
        let scale = fit * f64::from(view.zoom);
        let footprint = 1. / scale;
        let mut result = Vec::new();
        for py in 0..64 {
            for px in 0..64 {
                let x = (f64::from(px) + 0.5 - 32. - f64::from(view.pan[0])) / scale + f64::from(w) / 2.;
                let y = (f64::from(py) + 0.5 - 32. - f64::from(view.pan[1])) / scale + f64::from(h) / 2.;
                if x < 0. || y < 0. || x >= f64::from(w) || y >= f64::from(h) {
                    result.push([background; 3]);
                    continue;
                }
                let left = (x - footprint / 2.).max(0.);
                let right = (x + footprint / 2.).min(f64::from(w));
                let top = (y - footprint / 2.).max(0.);
                let bottom = (y + footprint / 2.).min(f64::from(h));
                let mut sum = [0.0f64; 4];
                let mut total = 0.;
                for sy in top.floor() as u32..bottom.ceil() as u32 {
                    for sx in left.floor() as u32..right.ceil() as u32 {
                        let weight = (right.min(f64::from(sx + 1)) - left.max(f64::from(sx)))
                            * (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy)));
                        let pixel = source[(sy * w + sx) as usize];
                        let alpha = f64::from(pixel[3]);
                        for channel in 0..3 {
                            sum[channel] += f64::from(pixel[channel]) * alpha * weight;
                        }
                        sum[3] += alpha * weight;
                        total += weight;
                    }
                }
                result.push(std::array::from_fn(|channel| {
                    sum[channel] / total * 2.0f64.powf(f64::from(view.exposure_stops))
                        + background * (1. - sum[3] / total)
                }));
            }
        }
        result
    }
    let size = [73u32, 69u32];
    let pixels: Vec<[f32; 4]> = (0..size[0] * size[1])
        .map(|i| {
            let x = i % size[0];
            let y = i / size[0];
            let alpha = [0., 0.25, 0.5, 1.][((x + 2 * y) % 4) as usize];
            if alpha == 0. {
                [1e6, -1e6, 9e5, 0.]
            } else {
                [
                    1.25 + (x % 9) as f32 / 4.,
                    (x % 5) as f32 / 4. - 0.5,
                    0.125 + (y % 7) as f32 / 8.,
                    alpha,
                ]
            }
        })
        .collect();
    let clean: Vec<[f32; 4]> = pixels
        .iter()
        .map(|p| if p[3] == 0. { [0.; 4] } else { *p })
        .collect();
    let source = DecodedRaster::new(
        size[0],
        size[1],
        RasterPixels::Rgba32Float(Arc::new(pixels.iter().flatten().copied().collect::<Vec<_>>()).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let cleaned = DecodedRaster::new(
        size[0],
        size[1],
        RasterPixels::Rgba32Float(Arc::new(clean.into_iter().flatten().collect::<Vec<_>>()).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    for tiled in [false, true] {
        let Some(mut gpu) = common::qualification_gpu() else {
            return;
        };
        if tiled {
            let instance = common::headless_instance();
            let adapter =
                pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
            let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits {
                    max_texture_dimension_2d: 64,
                    ..Default::default()
                },
                ..Default::default()
            }))
            .unwrap();
            gpu.device = device;
            gpu.queue = queue;
        }
        let mut maximum_error = 0.0f64;
        for zoom in [0.37, 0.63, 0.91] {
            let view = ViewParameters {
                viewport: [64., 64.],
                zoom,
                pan: [0.375, -0.25],
                exposure_stops: 0.75,
            };
            let reference = expected(&pixels, size, view);
            for hdr in [false, true] {
                let budget = rrrah_core::MemoryBudget::new(
                    rrrah_gpu::RasterRenderer::texture_bytes_for_extent(&gpu.device, size[0], size[1])
                        .unwrap()
                        * 2,
                );
                let mut renderer = if hdr {
                    rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone())
                } else {
                    rrrah_gpu::RasterRenderer::new_with_budget(
                        &gpu.device,
                        common::READBACK_FORMAT,
                        budget.clone(),
                    )
                };
                renderer.upload(&gpu.device, &gpu.queue, &source).unwrap();
                renderer.set_area_minification(&gpu.queue, true);
                renderer.update_view(&gpu.queue, view);
                let actual: Vec<u16> = if hdr {
                    gpu.render_existing_raster_half(&renderer, [64, 64])
                } else {
                    gpu.render_existing_raster(&renderer, [64, 64])
                        .pixels
                        .into_iter()
                        .map(u16::from)
                        .collect()
                };
                renderer.upload(&gpu.device, &gpu.queue, &cleaned).unwrap();
                renderer.set_area_minification(&gpu.queue, true);
                renderer.update_view(&gpu.queue, view);
                let no_hidden: Vec<u16> = if hdr {
                    gpu.render_existing_raster_half(&renderer, [64, 64])
                } else {
                    gpu.render_existing_raster(&renderer, [64, 64])
                        .pixels
                        .into_iter()
                        .map(u16::from)
                        .collect()
                };
                assert_eq!(
                    actual, no_hidden,
                    "invisible RGB tiled={tiled} zoom={zoom} hdr={hdr}"
                );
                for (index, pixel) in actual.chunks_exact(4).enumerate() {
                    assert_eq!(pixel[3], if hdr { 0x3c00 } else { 255 });
                    for channel in 0..3 {
                        let target = reference[index][channel];
                        if hdr {
                            let value = half_value(pixel[channel]);
                            let error = (value - target).abs();
                            maximum_error = maximum_error.max(error);
                            let half_step = if target.abs() < 2.0f64.powi(-14) {
                                2.0f64.powi(-24)
                            } else {
                                2.0f64.powi(target.abs().log2().floor() as i32 - 10)
                            };
                            // Format quantization plus conservative f32 geometry budget for this73-pixel control.
                            let geometry = 8. * f64::from(f32::EPSILON) * 73. * 5. * 2.0f64.powf(0.75);
                            assert!(
                                error <= half_step + geometry,
                                "HDR tiled={tiled} zoom={zoom} pixel={index} ch={channel} error={error}"
                            );
                        } else {
                            let linear = target.clamp(0., 1.);
                            let encoded = if linear <= 0.0031308 {
                                linear * 12.92
                            } else {
                                1.055 * linear.powf(1. / 2.4) - 0.055
                            };
                            let byte = (encoded * 255.).round() as u16;
                            assert!(
                                pixel[channel].abs_diff(byte) <= 1,
                                "SDR tiled={tiled} zoom={zoom} pixel={index} ch={channel}"
                            );
                        }
                    }
                }
                drop(renderer);
                assert_eq!(budget.used(), 0);
            }
        }
        eprintln!(
            "fractional area f64: tiled={tiled} max_hdr_abs_error={maximum_error}; three zooms, clipped edges, alpha0/.25/.5/1, invisible RGB bit-exact, CPU oracle within format/geometry precision, credits zero"
        );
    }
}

#[test]
fn area_large_signed_hdr_cancellation_preserves_small_other_channels() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let source = DecodedRaster::new(
        4,
        4,
        RasterPixels::Rgba32Float(
            Arc::new(
                (0..16)
                    .flat_map(|i| [if i % 4 < 2 { f32::MAX } else { -f32::MAX }, 1., 0.25, 1.])
                    .collect::<Vec<_>>(),
            )
            .into(),
        ),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let budget = rrrah_core::MemoryBudget::new(4 * 4 * 16);
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&gpu.device, budget.clone());
    renderer.upload(&gpu.device, &gpu.queue, &source).unwrap();
    renderer.set_area_minification(&gpu.queue, true);
    renderer.update_view(
        &gpu.queue,
        ViewParameters {
            viewport: [64., 64.],
            zoom: 0.03125,
            pan: [0.5, 0.5],
            ..Default::default()
        },
    );
    let frame = gpu.render_existing_raster_half(&renderer, [64, 64]);
    let pixel = &frame[(32 * 64 + 32) * 4..(32 * 64 + 32 + 1) * 4];
    assert_eq!(
        pixel,
        [0, 0x3c00, 0x3400, 0x3c00],
        "f64 mean of +/-f32MAX is0; green/blue remain1/.25"
    );
    drop(renderer);
    assert_eq!(budget.used(), 0);
}
