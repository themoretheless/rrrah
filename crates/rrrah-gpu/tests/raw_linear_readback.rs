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
        for (rgbe, above_file_white) in [(false, false), (true, false), (false, true)] {
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
                if above_file_white { 3967. } else { 1000. },
                if rgbe || above_file_white {
                    [1.; 4]
                } else {
                    [2., 1., 0.5, 1.]
                },
                profile,
                |x, y| {
                    if rgbe {
                        [3000, 0, 0, 100][((y % 2) * 2 + x % 2) as usize]
                    } else if above_file_white {
                        3989
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
            if above_file_white {
                source.metadata.black_level = rrrah_core::LevelGrid {
                    width: 2,
                    height: 2,
                    components: 1,
                    values: vec![15.; 4],
                };
            }
            let expected = if above_file_white {
                // Independent normalized sample with original GH2 file levels:
                // encoded3989 exceeds declared3967; do not clip to unity.
                [(3989. - 15.) / (3967. - 15.); 3]
            } else if rgbe {
                // Independent LibRaw F828 matrix times R=0,G=.1,B=0,E=3.
                [
                    -0.252761811 * 0.1 - 0.38109079 * 3.,
                    0.8223665357 * 0.1 + 0.6410246491 * 3.,
                    -0.3551472425 * 0.1 - 0.05928355828 * 3.,
                ]
            } else {
                [6., 3., 1.5]
            };
            let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
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
            if rgbe {
                let mut metadata = source.metadata.clone();
                metadata.black_level = rrrah_core::LevelGrid {
                    width: 3,
                    height: 5,
                    components: 1,
                    values: (0..15).map(|i| (i * 20) as f32).collect(),
                };
                let pixels: Vec<u16> = (0..16)
                    .flat_map(|y| {
                        (0..16).map(move |x| {
                            let black = ((y % 5) * 3 + x % 3) * 20;
                            let normalized_numerator = [3000, 0, 0, 100][((y % 2) * 2 + x % 2) as usize];
                            (black + normalized_numerator * (1000 - black) / 1000) as u16
                        })
                    })
                    .collect();
                let repeated = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(pixels)).unwrap();
                renderer.upload_mosaic(&device, &queue, &repeated).unwrap();
                let corrected = read_center(&device, &queue, &renderer, format);
                for c in 0..3 {
                    assert!(
                        (corrected[c] - actual[c]).abs() < 0.003,
                        "RGBE odd grid: {corrected:?} vs {actual:?}"
                    );
                }
                renderer.upload_mosaic(&device, &queue, &source).unwrap();
                eprintln!(
                    "RAW RGBE repeat grid: {format:?}; 3x5; four planes and independent signed HDR matrix preserved"
                );
            }
            if above_file_white {
                assert!(actual[..3].iter().all(|value| *value > 2.0));
                eprintln!(
                    "RAW above file white: {format:?}; encoded=3989; black=15; white=3967; exposure=1; all channels retain HDR headroom"
                );
                let retained = budget.used();
                for phase in 0..4 {
                    for invalid_black in [3967., 3968.] {
                        let mut malformed = source.clone();
                        malformed.metadata.black_level.values[phase] = invalid_black;
                        assert!(matches!(
                            renderer.upload_mosaic(&device, &queue, &malformed),
                            Err(rrrah_gpu::GpuError::InvalidLevels { .. })
                        ));
                        assert_eq!(budget.used(), retained);
                        assert_eq!(
                            read_center(&device, &queue, &renderer, format),
                            actual,
                            "invalid levels must preserve the last valid frame: phase={phase}"
                        );
                    }
                }
                eprintln!(
                    "RAW invalid levels: {format:?}; phases=4; equal/inverted refused; previous HDR frame exact; credit unchanged"
                );
                for length in [0, 255, 257] {
                    let mut malformed = source.clone();
                    malformed.pixels = std::sync::Arc::new(vec![3989_u16; length]).into();
                    let error = renderer.upload_mosaic(&device, &queue, &malformed).unwrap_err();
                    assert!(
                        matches!(error, rrrah_gpu::GpuError::Frame(
                        rrrah_core::FrameError::InvalidPixelCount { expected: 256, actual }
                    ) if actual == length),
                        "{error:?}"
                    );
                    assert_eq!(budget.used(), retained);
                    assert_eq!(read_center(&device, &queue, &renderer, format), actual);
                }
                eprintln!(
                    "RAW invalid pixel count: {format:?}; empty/truncated/extra refused; prior frame and credit exact"
                );
                let levels: Vec<f32> = (0..15).map(|i| (i * 20) as f32).collect();
                let mut repeated =
                    common::profiled_pattern_mosaic(16, 16, 1000., [1.; 4], profile, |x, y| {
                        (500. + levels[((y % 5) * 3 + x % 3) as usize] / 2.) as u16
                    });
                repeated.metadata.black_level = rrrah_core::LevelGrid {
                    width: 3,
                    height: 5,
                    components: 1,
                    values: levels,
                };
                let pressure = budget.try_reserve(4 * 1024 * 1024).unwrap();
                assert!(renderer.upload_mosaic(&device, &queue, &repeated).is_err());
                assert_eq!(read_center(&device, &queue, &renderer, format), actual);
                assert_eq!(budget.used(), retained + 4 * 1024 * 1024);
                drop(pressure);
                renderer.upload_mosaic(&device, &queue, &repeated).unwrap();
                assert_eq!(budget.used(), retained + 15 * 4);
                let corrected = read_center(&device, &queue, &renderer, format);
                // Every sensor site is exactly (encoded-black)/(1000-black)=.5;
                // identity color and one stop of exposure must yield RGB=1.
                for value in &corrected[..3] {
                    assert!((*value - 1.).abs() < 0.002, "{corrected:?}");
                }
                let mut invalid = repeated.clone();
                invalid.metadata.black_level.values[14] = 1000.;
                assert!(matches!(
                    renderer.upload_mosaic(&device, &queue, &invalid),
                    Err(rrrah_gpu::GpuError::InvalidLevels { .. })
                ));
                assert_eq!(budget.used(), retained + 60);
                assert_eq!(read_center(&device, &queue, &renderer, format), corrected);
                let mut fractional =
                    common::profiled_pattern_mosaic(16, 16, 1.25, [1.; 4], profile, |_, _| 1);
                fractional.metadata.black_level.values[0] = 0.75;
                renderer.upload_mosaic(&device, &queue, &fractional).unwrap();
                let fractional_frame = read_center(&device, &queue, &renderer, format);
                // Independent arithmetic: (1-.75)/(1.25-.75) * 2 = 1.
                for value in &fractional_frame[..3] {
                    assert!(
                        (*value - 1.).abs() < 0.002,
                        "fractional RAW range: {fractional_frame:?}"
                    );
                }
                eprintln!(
                    "RAW fractional levels: {format:?}; black=.75 white=1.25 sample=1 exposure=1; RGB=1"
                );
                let mut zero_edge =
                    common::profiled_pattern_mosaic(16, 16, f32::from_bits(1), [1.; 4], profile, |_, _| 0);
                renderer.upload_mosaic(&device, &queue, &zero_edge).unwrap();
                let zero_frame = read_center(&device, &queue, &renderer, format);
                assert_eq!(zero_frame, [0., 0., 0., 1.]);
                let edge_credit = budget.used();
                let checks = std::cell::Cell::new(0);
                assert!(matches!(
                    renderer.upload_mosaic_with_cancel(&device, &queue, &zero_edge, || {
                        let n = checks.get() + 1;
                        checks.set(n);
                        n >= 2
                    }),
                    Err(rrrah_gpu::GpuError::Cancelled)
                ));
                assert_eq!(checks.get(), 2);
                assert_eq!(budget.used(), edge_credit);
                assert_eq!(read_center(&device, &queue, &renderer, format), zero_frame);

                zero_edge.pixels = std::sync::Arc::new(vec![1_u16; 256]).into();
                assert!(matches!(
                    renderer.upload_mosaic(&device, &queue, &zero_edge),
                    Err(rrrah_gpu::GpuError::NonFiniteSensorCorrection { .. })
                ));
                assert_eq!(budget.used(), edge_credit);
                assert_eq!(read_center(&device, &queue, &renderer, format), zero_frame);
                zero_edge.metadata.white_level.0[0] = 1.;
                zero_edge.metadata.white_balance = [f32::MAX; 4];
                zero_edge.pixels = std::sync::Arc::new(vec![2_u16; 256]).into();
                assert!(matches!(
                    renderer.upload_mosaic(&device, &queue, &zero_edge),
                    Err(rrrah_gpu::GpuError::NonFiniteSensorCorrection { .. })
                ));
                assert_eq!(budget.used(), edge_credit);
                assert_eq!(read_center(&device, &queue, &renderer, format), zero_frame);
                renderer.upload_mosaic(&device, &queue, &fractional).unwrap();
                assert_eq!(read_center(&device, &queue, &renderer, format), fractional_frame);
                eprintln!(
                    "RAW extreme correction: {format:?}; subnormal zero field exact; normalization/gain overflow refused; prior frame and credit exact; valid retry"
                );
                let fractional_credit = budget.used();
                let mut overflowing = fractional.clone();
                overflowing.metadata.black_level.values[0] = -f32::MAX;
                overflowing.metadata.white_level.0[0] = f32::MAX;
                assert!(matches!(
                    renderer.upload_mosaic(&device, &queue, &overflowing),
                    Err(rrrah_gpu::GpuError::InvalidLevels { .. })
                ));
                assert_eq!(budget.used(), fractional_credit);
                assert_eq!(read_center(&device, &queue, &renderer, format), fractional_frame);

                renderer.upload_mosaic(&device, &queue, &source).unwrap();
                assert_eq!(budget.used(), retained);
                assert_eq!(read_center(&device, &queue, &renderer, format), actual);
                eprintln!(
                    "RAW repeat grid: {format:?}; 3x5 odd boundaries; independent neutral exact; extra credit=60; invalid last cell preserves frame; fast path releases grid"
                );
            }
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

#[test]
fn finite_large_bayer_values_refuse_unsafe_filter_range_before_replacement() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let profile = [inverse[0], inverse[1], inverse[2], [0.; 3]];
    let valid = common::profiled_pattern_mosaic(16, 16, 1., [1.; 4], profile, |_, _| 1);
    let extreme = common::profiled_pattern_mosaic(16, 16, 1., [1e38; 4], profile, |_, _| 1);
    for format in [wgpu::TextureFormat::Rgba32Float, wgpu::TextureFormat::Rgba16Float] {
        let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
        let mut renderer = RawRenderer::new_with_budget(&device, format, budget.clone());
        renderer
            .set_output_mode(&queue, RawOutputMode::SceneLinear)
            .unwrap();
        renderer.upload_mosaic(&device, &queue, &valid).unwrap();
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 2.,
                ..Default::default()
            },
        );
        let before = read_center(&device, &queue, &renderer, format);
        let retained = budget.used();
        assert!(matches!(
            renderer.upload_mosaic(&device, &queue, &extreme),
            Err(rrrah_gpu::GpuError::ShaderFilterRangeExceeded { .. })
        ));
        assert_eq!(read_center(&device, &queue, &renderer, format), before);
        renderer.upload_mosaic(&device, &queue, &valid).unwrap();
        assert_eq!(read_center(&device, &queue, &renderer, format), before);
        let unsafe_color = common::profiled_pattern_mosaic(16, 16, 1., [1e16; 4], profile, |_, _| 1);
        assert!(matches!(
            renderer.upload_mosaic(&device, &queue, &unsafe_color),
            Err(rrrah_gpu::GpuError::ShaderFilterRangeExceeded { .. })
        ));
        assert_eq!(read_center(&device, &queue, &renderer, format), before);
        assert_eq!(budget.used(), retained);
        let zero = common::profiled_pattern_mosaic(16, 16, 1., [1e16; 4], profile, |_, _| 0);
        for cutoff in [2, 9, 17] {
            let checks = std::cell::Cell::new(0);
            assert!(matches!(
                renderer.upload_mosaic_with_cancel(&device, &queue, &zero, || {
                    let next = checks.get() + 1;
                    checks.set(next);
                    next >= cutoff
                }),
                Err(rrrah_gpu::GpuError::Cancelled)
            ));
            assert_eq!(checks.get(), cutoff);
            assert_eq!(budget.used(), retained);
            assert_eq!(read_center(&device, &queue, &renderer, format), before);
        }
        renderer.upload_mosaic(&device, &queue, &zero).unwrap();
        assert_eq!(read_center(&device, &queue, &renderer, format), [0., 0., 0., 1.]);
        assert_eq!(budget.used(), retained);
        renderer.upload_mosaic(&device, &queue, &valid).unwrap();
        assert_eq!(read_center(&device, &queue, &renderer, format), before);
        let bright = common::profiled_pattern_mosaic(16, 16, 1., [100.; 4], profile, |_, _| 1);
        renderer.upload_mosaic(&device, &queue, &bright).unwrap();
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 2.,
                exposure_stops: 10.,
                ..Default::default()
            },
        );
        let actual = read_center(&device, &queue, &renderer, format);
        for value in &actual[..3] {
            if format == wgpu::TextureFormat::Rgba16Float {
                assert_eq!(*value, 65504.);
            } else {
                assert!(value.is_finite() && (*value - 102400.).abs() < 1.);
            }
        }
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn nonfinite_raw_view_components_keep_previous_finite_frame() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let source = common::profiled_pattern_mosaic(
        16,
        16,
        1000.,
        [1.; 4],
        [inverse[0], inverse[1], inverse[2], [0.; 3]],
        |_, _| 500,
    );
    for format in [wgpu::TextureFormat::Rgba16Float, wgpu::TextureFormat::Rgba32Float] {
        let mut renderer = RawRenderer::new(&device, format);
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
        let before = read_center(&device, &queue, &renderer, format);
        assert!(before.iter().all(|v| v.is_finite()));
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            renderer.update_view(
                &queue,
                ViewParameters {
                    viewport: [invalid; 2],
                    pan: [invalid; 2],
                    zoom: invalid,
                    exposure_stops: invalid,
                },
            );
            assert_eq!(read_center(&device, &queue, &renderer, format), before);
        }
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [f32::NAN; 2],
                pan: [f32::NAN; 2],
                zoom: f32::NAN,
                exposure_stops: 0.,
            },
        );
        let mixed = read_center(&device, &queue, &renderer, format);
        assert_ne!(mixed, before);
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 2.,
                exposure_stops: 0.,
                ..Default::default()
            },
        );
        assert_eq!(read_center(&device, &queue, &renderer, format), mixed);
    }
}

#[test]
fn signed_rgbe_hdr_saturation_preserves_float32_and_both_half_limits() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let profile = [
        [0.7924, -0.1910, -0.0777],
        [-0.8226, 1.5459, 0.2998],
        [-0.1517, 0.2199, 0.6818],
        [-0.7242, 1.1401, 0.3481],
    ];
    let mut source = common::profiled_pattern_mosaic(16, 16, 1000., [100.; 4], profile, |x, y| {
        [3000, 0, 0, 100][((y % 2) * 2 + x % 2) as usize]
    });
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
    for zoom in [2., 0.3, 0.125] {
        let view = ViewParameters {
            viewport: [64., 64.],
            zoom,
            exposure_stops: 10.,
            ..Default::default()
        };
        let mut wide = RawRenderer::new(&device, wgpu::TextureFormat::Rgba32Float);
        wide.set_output_mode(&queue, RawOutputMode::SceneLinear).unwrap();
        wide.upload_mosaic(&device, &queue, &source).unwrap();
        wide.update_view(&queue, view);
        let linear = read_center(&device, &queue, &wide, wgpu::TextureFormat::Rgba32Float);
        assert!(linear.iter().all(|v| v.is_finite()), "{linear:?}");
        assert!(linear[..3].iter().any(|v| *v > 65504.), "{linear:?}");
        assert!(linear[..3].iter().any(|v| *v < -65504.), "{linear:?}");
        let mut half = RawRenderer::new(&device, wgpu::TextureFormat::Rgba16Float);
        half.set_output_mode(&queue, RawOutputMode::SceneLinear).unwrap();
        half.upload_mosaic(&device, &queue, &source).unwrap();
        half.update_view(&queue, view);
        let actual = read_center(&device, &queue, &half, wgpu::TextureFormat::Rgba16Float);
        for channel in 0..3 {
            let expected = linear[channel].clamp(-65504., 65504.);
            assert!(actual[channel].is_finite());
            if expected.abs() == 65504. {
                assert_eq!(actual[channel], expected);
            } else {
                assert!((actual[channel] - expected).abs() <= expected.abs() * 0.001 + 0.001);
            }
        }
    }
}

#[test]
fn raw_renderer_5x5_gpu_demosaic_algorithms_auto_raise_tile_halo_and_preserve_linear_hdr() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let source = common::profiled_pattern_mosaic(
        64,
        64,
        1000.,
        [2., 1., 0.5, 1.],
        [inverse[0], inverse[1], inverse[2], [0.; 3]],
        |_, _| 3000,
    );
    for alg in [
        rrrah_gpu::GpuDemosaicAlgorithm::Bilinear,
        rrrah_gpu::GpuDemosaicAlgorithm::MalvarHeCutler,
        rrrah_gpu::GpuDemosaicAlgorithm::Directional,
    ] {
        let mut renderer = RawRenderer::new(&device, wgpu::TextureFormat::Rgba32Float);
        renderer
            .set_output_mode(&queue, RawOutputMode::SceneLinear)
            .unwrap();
        renderer.set_demosaic_algorithm(&queue, alg);
        assert_eq!(renderer.demosaic_algorithm(), alg);
        // Use small 32x32 tiles so the center of the 64x64 frame sits right on a 4-tile boundary,
        // exercising the automatic tile_halo >= 2 elevation for 5x5 GPU demosaic shaders.
        renderer
            .upload_mosaic_with_tiling(
                &device,
                &queue,
                &source,
                rrrah_gpu::TilingOverrides {
                    tile_size: Some(32),
                    tile_halo: None,
                },
            )
            .unwrap();
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [64., 64.],
                zoom: 2.,
                exposure_stops: 1.,
                ..Default::default()
            },
        );
        let actual = read_center(&device, &queue, &renderer, wgpu::TextureFormat::Rgba32Float);
        let expected = [12.0f32, 6.0, 3.0];
        for c in 0..3 {
            assert!(
                (actual[c] - expected[c]).abs() <= 5e-5,
                "{alg:?} channel {c}: actual={} expected={}",
                actual[c],
                expected[c]
            );
        }
        assert_eq!(actual[3], 1.0);
    }
}
