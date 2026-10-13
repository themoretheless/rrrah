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
        let budget = rrrah_core::MemoryBudget::new(bytes + 16);
        let uploads = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut renderer = rrrah_gpu::RasterRenderer::new_with_budget(
            &tiled.device,
            common::READBACK_FORMAT,
            budget.clone(),
        )
        .with_upload_queue_budget(uploads.clone());
        renderer
            .upload(&tiled.device, &tiled.queue, &frame(1, vec![1., 0., 0., 1.]))
            .unwrap();
        renderer.update_view(&tiled.queue, view());
        let before = tiled.render_existing_raster(&renderer, [64, 64]).pixels;
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            renderer.upload_with_cancel(&tiled.device, &tiled.queue, &raster, || {
                if budget.used() > 16 {
                    calls.set(calls.get() + 1);
                }
                calls.get() == 3
            }),
            Err(rrrah_gpu::RasterUploadError::Cancelled)
        ));
        tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 16);
        assert_eq!(uploads.used(), 0);
        assert_eq!(tiled.render_existing_raster(&renderer, [64, 64]).pixels, before);
        renderer.upload(&tiled.device, &tiled.queue, &raster).unwrap();
        tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(renderer.resident_bytes(), bytes);
        assert_eq!(budget.used(), bytes);
        drop(renderer);
        tiled.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
        assert_eq!(uploads.used(), 0);
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
    assert_eq!(budget.used(), 16000000);
    drop(lod);
    assert_eq!(budget.used(), 0);
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
