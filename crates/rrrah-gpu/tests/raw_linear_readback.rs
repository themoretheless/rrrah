mod common;
use rrrah_gpu::{RawOutputMode, RawRenderer, ViewParameters};
#[test]
fn raw_linear_float_target_preserves_hdr_and_negative_rgbe_color() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("RAW linear adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut sdr = RawRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert!(sdr.set_output_mode(&queue, RawOutputMode::SceneLinear).is_err());
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    for format in [wgpu::TextureFormat::Rgba16Float, wgpu::TextureFormat::Rgba32Float] {
        for rgbe in [false, true] {
            let profile = if rgbe {
                [
                    [0.7924, -0.1910, -0.0777],
                    [-0.8226, 1.5459, 0.2998],
                    [-0.1517, 0.2199, 0.6818],
                    [-0.7242, 1.1401, 0.3481],
                ]
            } else {
                [inverse[0], inverse[1], inverse[2], [0.; 3]]
            };
            let mut source = common::profiled_pattern_mosaic(
                16,
                16,
                1000.,
                if rgbe { [1.; 4] } else { [2., 1., 0.5, 1.] },
                profile,
                |x, y| {
                    if rgbe {
                        [3000, 0, 0, 100][((y % 2) * 2 + x % 2) as usize]
                    } else {
                        3000
                    }
                },
            );
            if rgbe {
                source.metadata.cfa = Some(rrrah_core::CfaPattern {
                    width: 2,
                    height: 2,
                    cells: vec![
                        rrrah_core::CfaColor::Emerald,
                        rrrah_core::CfaColor::Red,
                        rrrah_core::CfaColor::Blue,
                        rrrah_core::CfaColor::Green,
                    ],
                });
            }
            let expected = if rgbe {
                // Independent LibRaw F828 matrix times R=0,G=.1,B=0,E=3.
                [
                    -0.252761811 * 0.1 - 0.38109079 * 3.,
                    0.8223665357 * 0.1 + 0.6410246491 * 3.,
                    -0.3551472425 * 0.1 - 0.05928355828 * 3.,
                ]
            } else {
                [6., 3., 1.5]
            };
            let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
            let mut renderer = RawRenderer::new_with_budget(&device, format, budget.clone());
            renderer
                .set_output_mode(&queue, RawOutputMode::SceneLinear)
                .unwrap();
            renderer.upload_mosaic(&device, &queue, &source).unwrap();
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [64., 64.],
                    zoom: 2.,
                    exposure_stops: 1.,
                    ..Default::default()
                },
            );
            let actual = read_center(&device, &queue, &renderer, format);
            for c in 0..3 {
                assert!(
                    (f64::from(actual[c]) - expected[c] * 2.).abs()
                        <= if format == wgpu::TextureFormat::Rgba16Float {
                            // One binary16 ULP at the expected magnitude, plus
                            // the separately qualified f32 shader error bound.
                            // Metal attachment conversion can choose the adjacent
                            // representable value for an almost-exact f32 input.
                            let magnitude = (expected[c] * 2.).abs();
                            2f64.powi(magnitude.log2().floor() as i32 - 10)
                                .max(2f64.powi(-24))
                                + 2e-5
                        } else {
                            2e-5
                        },
                    "{format:?} RGBE {rgbe} channel {c}: {actual:?}"
                );
            }
            assert_eq!(actual[3], 1.);
            renderer
                .set_output_mode(&queue, RawOutputMode::DisplaySdr)
                .unwrap();
            let mapped = read_center(&device, &queue, &renderer, format);
            assert!(
                mapped[..3].iter().all(|v| *v >= -1e-6 && *v <= 1.000001),
                "SDR roundoff: {mapped:?}"
            );
            renderer
                .set_output_mode(&queue, RawOutputMode::SceneLinear)
                .unwrap();
            assert_eq!(read_center(&device, &queue, &renderer, format), actual);
            drop(renderer);
            assert_eq!(budget.used(), 0);
        }
    }
}
fn read_center(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &RawRenderer,
    format: wgpu::TextureFormat,
) -> [f32; 4] {
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
        format,
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
    let result: [f32; 4] = if format == wgpu::TextureFormat::Rgba16Float {
        let words: &[u16] = bytemuck::cast_slice(&mapped[..8]);
        std::array::from_fn(|i| decode_half(words[i]))
    } else {
        let channels: &[f32] = bytemuck::cast_slice(&mapped[..16]);
        channels.try_into().unwrap()
    };
    drop(mapped);
    readback.unmap();
    result
}

// Independent IEEE-754 binary16 expansion; no production conversion helper.
fn decode_half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (bits >> 10) & 31;
    let fraction = f32::from(bits & 1023);
    match exponent {
        0 => sign * fraction * 2f32.powi(-24),
        31 => {
            if fraction == 0. {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1. + fraction / 1024.) * 2f32.powi(i32::from(exponent) - 15),
    }
}
#[test]
fn binary16_reference_covers_signed_hdr_and_subnormals() {
    for (bits, expected) in [
        (0x4800, 8.),
        (0x4400, 4.),
        (0xbc00, -1.),
        (0x3c00, 1.),
        (1, 2f32.powi(-24)),
        (0x7bff, 65504.),
    ] {
        assert_eq!(decode_half(bits), expected);
    }
    assert_eq!(decode_half(0x8000).to_bits(), (-0f32).to_bits());
}
