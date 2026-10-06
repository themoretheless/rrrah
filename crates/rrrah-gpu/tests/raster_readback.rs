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
    let dimensions = std::env::var("RRRAH_X3F_EXPECTED_DIMENSIONS")
        .unwrap_or_else(|_| "2267x1513".into());
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
