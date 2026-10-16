//! Opt-in local camera control; no original media or rendered pixels are exported.
mod common;
#[path = "common/bayer_directional_reference.rs"]
mod reference;

#[test]
#[ignore = "requires local EOS R8 CR3 and actual GPU; sampled f64 reconstruction oracle"]
fn original_cr3_all_compute_algorithms_match_sampled_f64_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
    let mosaic = rrrah_decode::decode_file(&path).unwrap().mosaic;
    let [w, h] = [mosaic.metadata.width, mosaic.metadata.height];
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    eprintln!("real Bayer oracle adapter: {:?}", adapter.get_info());
    let mut limits = wgpu::Limits::default();
    limits.max_buffer_size = u64::from(w) * u64::from(h) * 16;
    assert!(limits.max_buffer_size <= adapter.limits().max_buffer_size);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: limits,
        ..Default::default()
    }))
    .unwrap();
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&device);
    let gpu_budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
    let cpu_budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut points = Vec::new();
    for y in [0, 1, 2, h / 4, h / 2, h * 3 / 4, h - 3, h - 2, h - 1] {
        for x in [0, 1, 2, w / 4, w / 2, w * 3 / 4, w - 3, w - 2, w - 1] {
            points.push([x, y]);
        }
    }
    for i in 0..256 {
        points.push([(i * 7919) % w, (i * 3571) % h]);
    }
    points.sort_unstable();
    points.dedup();
    for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
        let resident = compute
            .execute_mosaic_resident(&queue, &mosaic, algorithm, &gpu_budget, &cpu_budget, || false)
            .unwrap();
        let bytes = points.len() as u64 * 16;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sampled original sensor reconstruction"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        for (i, &[x, y]) in points.iter().enumerate() {
            let offset = (u64::from(y) * u64::from(w) + u64::from(x)) * 16;
            encoder.copy_buffer_to_buffer(resident.buffer(), offset, &readback, i as u64 * 16, 16);
        }
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let actual: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
        let mut maximum = 0f64;
        for (&[x, y], pixel) in points.iter().zip(actual) {
            let expected = reference::weighted_rgb(&mosaic, x, y, algorithm);
            assert_eq!(pixel[3], 1.);
            for c in 0..3 {
                let error = (f64::from(pixel[c]) - expected[c]).abs();
                maximum = maximum.max(error);
                let bound = 32. * f64::from(f32::EPSILON) * expected[c].abs().max(1.);
                assert!(
                    pixel[c].is_finite() && error <= bound,
                    "{algorithm:?} at{x},{y} channel{c}: actual={} reference={} error={error} bound={bound}",
                    pixel[c],
                    expected[c]
                );
            }
        }
        drop(mapped);
        readback.unmap();
        drop(readback);
        drop(resident);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!((gpu_budget.used(), cpu_budget.used()), (0, 0));
        eprintln!(
            "real CR3 sampled oracle: {algorithm:?} points={} max_abs_error={maximum} credits=0",
            points.len()
        );
    }
}

#[test]
#[ignore = "requires actual GPU; full-pixel f64 oracle across Bayer phases and odd small sensors"]
fn all_pixels_all_bayer_phases_and_algorithms_match_f64_reference() {
    use rrrah_core::{CfaColor, CfaPattern};
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    eprintln!("full Bayer oracle adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let compute = rrrah_gpu::BayerDemosaicCompute::new(&device);
    let gpu_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let cpu_budget = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut total = 0usize;
    let mut maximum = 0f64;
    for [width, height] in [[2, 2], [3, 5], [8, 7], [17, 19]] {
        for phase in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
            let mut mosaic = common::pattern_mosaic(width, height, 4095., |x, y| {
                ((x * 173 + y * 997 + x * y * 31) % 4096) as u16
            });
            mosaic.metadata.white_balance = [1.7, 1.0, 1.3, 1.25];
            mosaic.metadata.cfa = Some(CfaPattern {
                width: 2,
                height: 2,
                cells: phase
                    .map(|c| match c {
                        0 => CfaColor::Red,
                        1 => CfaColor::Green,
                        2 => CfaColor::Blue,
                        _ => unreachable!(),
                    })
                    .to_vec(),
            });
            for calibrated in [false, true] {
                if calibrated {
                    mosaic.metadata.black_level = rrrah_core::LevelGrid {
                        width: 3,
                        height: 2,
                        components: 1,
                        values: vec![17., 83., 149., 211., 277., 349.],
                    };
                    mosaic.metadata.white_level = rrrah_core::WhiteLevel(vec![3071., 3583., 4095., 4607.]);
                }
                for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
                    let resident = compute
                        .execute_mosaic_resident(&queue, &mosaic, algorithm, &gpu_budget, &cpu_budget, || {
                            false
                        })
                        .unwrap();
                    let bytes = u64::from(width) * u64::from(height) * 16;
                    let readback = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("full independent Bayer oracle"),
                        size: bytes,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });
                    let mut encoder = device.create_command_encoder(&Default::default());
                    encoder.copy_buffer_to_buffer(resident.buffer(), 0, &readback, 0, bytes);
                    queue.submit([encoder.finish()]);
                    let (tx, rx) = std::sync::mpsc::channel();
                    readback
                        .slice(..)
                        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    rx.recv().unwrap().unwrap();
                    let mapped = readback.slice(..).get_mapped_range().unwrap();
                    let actual: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
                    for y in 0..height {
                        for x in 0..width {
                            let expected = reference::weighted_rgb(&mosaic, x, y, algorithm);
                            let pixel = actual[(y * width + x) as usize];
                            assert_eq!(pixel[3], 1.);
                            for c in 0..3 {
                                let error = (f64::from(pixel[c]) - expected[c]).abs();
                                maximum = maximum.max(error);
                                let bound = 32. * f64::from(f32::EPSILON) * expected[c].abs().max(1.);
                                assert!(
                                    pixel[c].is_finite() && error <= bound,
                                    "{width}x{height} phase={phase:?} calibrated={calibrated} {algorithm:?} {x},{y}/{c}: actual={} expected={} error={error} bound={bound}",
                                    pixel[c],
                                    expected[c]
                                );
                            }
                            total += 1;
                        }
                    }
                    drop(mapped);
                    readback.unmap();
                    drop(readback);
                    drop(resident);
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    assert_eq!((gpu_budget.used(), cpu_budget.used()), (0, 0));
                }
            }
        }
    }
    eprintln!("FULL_BAYER_ORACLE cases=192 pixels={total} max_abs_error={maximum} credits=0");
}

#[test]
#[ignore = "local CR3 and actual GPU; compact64MiB atlas versus default full-frame SDR/half readback"]
fn original_cr3_capacity_fallback_preserves_default_atlas_pixels() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
    let mosaic = rrrah_decode::decode_file(&path).unwrap().mosaic;
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("capacity atlas adapter: {}", gpu.adapter_name());
    let size = [1280, 720];
    let mut readbacks = 0;
    for algorithm in rrrah_gpu::GpuDemosaicAlgorithm::ALL {
        for hdr in [false, true] {
            let format = if hdr {
                wgpu::TextureFormat::Rgba16Float
            } else {
                wgpu::TextureFormat::Rgba8UnormSrgb
            };
            let mut reference = rrrah_gpu::RawRenderer::new(&gpu.device, format);
            let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
            let mut compact = rrrah_gpu::RawRenderer::new_with_budget(&gpu.device, format, budget.clone());
            for renderer in [&mut reference, &mut compact] {
                renderer.set_demosaic_algorithm(&gpu.queue, algorithm);
                if hdr {
                    renderer
                        .set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
                        .unwrap();
                }
                renderer.upload_mosaic(&gpu.device, &gpu.queue, &mosaic).unwrap();
            }
            assert!(budget.used() > 0 && budget.used() <= budget.limit());
            for (zoom, pan) in [
                (1., [0., 0.]),
                (4., [0., 0.]),
                (4., [721., -339.]),
                (4., [-911., 477.]),
            ] {
                let view = rrrah_gpu::ViewParameters {
                    viewport: [1280., 720.],
                    zoom,
                    pan,
                    ..Default::default()
                };
                reference.update_view(&gpu.queue, view);
                compact.update_view(&gpu.queue, view);
                if hdr {
                    assert_eq!(
                        gpu.render_existing_raw_half(&compact, size),
                        gpu.render_existing_raw_half(&reference, size),
                        "{algorithm:?} zoom={zoom} pan={pan:?}"
                    );
                } else {
                    assert_eq!(
                        gpu.render_existing_raw(&compact, size).pixels,
                        gpu.render_existing_raw(&reference, size).pixels,
                        "{algorithm:?} zoom={zoom} pan={pan:?}"
                    );
                }
                readbacks += 2;
            }
            let retained = budget.used();
            assert!(matches!(
                compact.upload_mosaic(&gpu.device, &gpu.queue, &mosaic),
                Err(rrrah_gpu::GpuError::Memory(
                    rrrah_core::BufferError::Capacity { .. }
                ))
            ));
            assert_eq!(budget.used(), retained);
            assert!(matches!(
                compact.upload_mosaic_with_cancel(&gpu.device, &gpu.queue, &mosaic, || true),
                Err(rrrah_gpu::GpuError::Cancelled)
            ));
            assert_eq!(budget.used(), retained);
            if hdr {
                assert_eq!(
                    gpu.render_existing_raw_half(&compact, size),
                    gpu.render_existing_raw_half(&reference, size)
                );
            } else {
                assert_eq!(
                    gpu.render_existing_raw(&compact, size).pixels,
                    gpu.render_existing_raw(&reference, size).pixels
                );
            }
            readbacks += 2;
            eprintln!(
                "CAPACITY_ATLAS_PIXELS algorithm={algorithm:?} hdr={hdr} bytes={} peak={} exact=true",
                budget.used(),
                budget.peak()
            );
            drop(compact);
            assert_eq!(budget.used(), 0);
        }
    }
    eprintln!(
        "CAPACITY_ATLAS_FULL_FRAME readbacks={readbacks} pixels_per_frame=921600 all_exact=true final=0"
    );
}

#[test]
#[ignore = "local CR3 and actual GPU; alternating paired upload-and-GPU-completion measurements"]
fn original_cr3_capacity_atlas_paired_upload_measurement() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3");
    let mosaic = rrrah_decode::decode_file(&path).unwrap().mosaic;
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("capacity atlas timing adapter: {}", gpu.adapter_name());
    for algorithm in [
        rrrah_gpu::GpuDemosaicAlgorithm::Bilinear,
        rrrah_gpu::GpuDemosaicAlgorithm::Rcd,
    ] {
        let mut samples = [Vec::new(), Vec::new()];
        let mut sizes = [0u64; 2];
        for pair in 0..14 {
            for mode in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                let budget = rrrah_core::MemoryBudget::new(if mode == 0 {
                    128 * 1024 * 1024
                } else {
                    64 * 1024 * 1024
                });
                let mut renderer = rrrah_gpu::RawRenderer::new_with_budget(
                    &gpu.device,
                    wgpu::TextureFormat::Rgba16Float,
                    budget.clone(),
                );
                renderer.set_demosaic_algorithm(&gpu.queue, algorithm);
                renderer
                    .set_output_mode(&gpu.queue, rrrah_gpu::RawOutputMode::SceneLinear)
                    .unwrap();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let start = std::time::Instant::now();
                renderer.upload_mosaic(&gpu.device, &gpu.queue, &mosaic).unwrap();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let elapsed = start.elapsed().as_nanos();
                sizes[mode] = budget.used();
                if pair >= 2 {
                    samples[mode].push(elapsed);
                }
                drop(renderer);
                assert_eq!(budget.used(), 0);
            }
        }
        assert!(sizes[0] > 64 * 1024 * 1024 && sizes[1] <= 64 * 1024 * 1024);
        eprintln!(
            "CAPACITY_UPLOAD_RAW algorithm={algorithm:?} default_ns={:?} compact_ns={:?} default_bytes={} compact_bytes={}",
            samples[0], samples[1], sizes[0], sizes[1]
        );
    }
}
