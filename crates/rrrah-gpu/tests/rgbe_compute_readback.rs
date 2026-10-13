mod common;
use wgpu::util::DeviceExt;
#[test]
fn four_plane_compute_matches_cpu_lattices_edges_hdr_and_tail() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("RGBE compute adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("RGBE interpolation"),
        source: wgpu::ShaderSource::Wgsl(rrrah_gpu::RGBE_INTERPOLATION_SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    for dimensions in [[2u32, 2], [3, 5], [17, 13], [17, 33], [16, 13]] {
        for quad in [[3u32, 0, 2, 1], [0, 3, 1, 2], [1, 2, 0, 3]] {
            let count = (dimensions[0] * dimensions[1]) as usize;
            let values: Vec<f32> = (0..count)
                .map(|i| {
                    let x = i as u32 % dimensions[0];
                    let y = i as u32 / dimensions[0];
                    let c = quad[((y % 2) * 2 + x % 2) as usize] as f32;
                    0.2 + c * 1.3 + x as f32 * (c + 1.) * 0.13 + y as f32 * (4. - c) * 0.09
                })
                .collect();
            let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&values),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let initial = vec![[91f32; 4]; count + 1];
            let output = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&initial),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: output.size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            let mut limits = device.limits();
            if dimensions[0] == 16 || dimensions[0] == 17 {
                limits.max_storage_buffer_binding_size = 1024;
            }
            let plan = rrrah_gpu::RgbePlan::new(dimensions, quad, &limits).unwrap();
            if dimensions[0] == 16 {
                assert_eq!(plan.dispatches.len(), 4);
            }
            if dimensions == [17, 13] {
                assert_eq!(plan.dispatches.len(), 5);
            }
            for dispatch in plan.dispatches {
                let params = [
                    dimensions[0],
                    dimensions[1],
                    dispatch.first_row,
                    dispatch.rows,
                    quad[0],
                    quad[1],
                    quad[2],
                    quad[3],
                    dispatch.output_first_pixel,
                    dispatch.source_first_sample,
                    0,
                    0,
                ];
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &source,
                                offset: dispatch.source_offset,
                                size: std::num::NonZeroU64::new(dispatch.source_bytes),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &output,
                                offset: dispatch.output_offset,
                                size: std::num::NonZeroU64::new(dispatch.output_bytes),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform.as_entire_binding(),
                        },
                    ],
                });
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(dispatch.workgroups[0], dispatch.workgroups[1], 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output.size());
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
            let actual: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
            for (i, pixel) in actual[..count].iter().enumerate() {
                let x = i as u32 % dimensions[0];
                let y = i as u32 / dimensions[0];
                let expected = rrrah_core::rgbe::interpolate_planes(quad, [x, y], dimensions, |sx, sy| {
                    values[(sy * dimensions[0] + sx) as usize] as f64
                })
                .unwrap();
                for (a, b) in pixel.iter().zip(expected) {
                    assert!(
                        (*a as f64 - b).abs() < 1e-5,
                        "{dimensions:?} {quad:?} pixel {i}: {a} != {b}"
                    );
                }
            }
            assert_eq!(actual[count], [91.; 4]);
            assert!(actual[..count].iter().flatten().any(|v| *v > 1.));
        }
    }
}

#[test]
fn segmented_input_cancellation_at_every_checkpoint_releases_and_retries() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    let dimensions = [17, 33];
    let quad = [3, 0, 2, 1];
    let source: Vec<f32> = (0..561).map(|i| (i % 29) as f32 * 0.125 - 1.).collect();
    let plan = rrrah_gpu::RgbePlan::new(dimensions, quad, &device.limits()).unwrap();
    assert!(plan.source_bytes > 1024);
    assert!(plan.dispatches.iter().any(|d| d.source_offset > 0));
    let cpu = rrrah_core::MemoryBudget::new(plan.output_bytes);
    let gpu = rrrah_core::MemoryBudget::new(plan.gpu_buffer_bytes);
    let compute = rrrah_gpu::RgbeCompute::new(&device);
    let mut total_checks = 0;
    let baseline = compute
        .execute_managed_with_cancel(&queue, &source, dimensions, quad, &gpu, &cpu, || {
            total_checks += 1;
            false
        })
        .unwrap();
    for (i, pixel) in baseline.iter().enumerate() {
        let expected = rrrah_core::rgbe::interpolate_planes(
            quad,
            [(i % 17) as u32, (i / 17) as u32],
            dimensions,
            |x, y| source[(y * 17 + x) as usize] as f64,
        )
        .unwrap();
        for (actual, expected) in pixel.iter().zip(expected) {
            assert!((*actual as f64 - expected).abs() < 1e-5);
        }
    }
    let expected = baseline.to_vec();
    drop(baseline);
    assert_eq!(total_checks, plan.dispatches.len() + 6);
    for checkpoint in 1..=total_checks {
        let mut checks = 0;
        assert!(matches!(
            compute.execute_managed_with_cancel(&queue, &source, dimensions, quad, &gpu, &cpu, || {
                checks += 1;
                checks == checkpoint
            },),
            Err(rrrah_gpu::RgbeComputeError::Cancelled)
        ));
        assert_eq!(checks, checkpoint);
        assert_eq!((cpu.used(), gpu.used()), (0, 0));
        let retry = compute
            .execute_managed(&queue, &source, dimensions, quad, &gpu, &cpu)
            .unwrap();
        assert_eq!(&*retry, expected.as_slice());
        drop(retry);
        assert_eq!((cpu.used(), gpu.used()), (0, 0));
    }
    eprintln!(
        "RGBE segmented cancellation: checkpoints={total_checks}; every retry exact; CPU/GPU final zero"
    );
}

#[test]
fn managed_rgbe_admission_completion_and_output_owner() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    let compute = rrrah_gpu::RgbeCompute::new(&device);
    let gpu = rrrah_core::MemoryBudget::new(8192);
    let cpu = rrrah_core::MemoryBudget::new(4096);
    for invalid in [vec![f32::NAN; 208], vec![f32::INFINITY; 208], vec![1.; 207]] {
        assert!(
            compute
                .execute_managed(&queue, &invalid, [16, 13], [3, 0, 2, 1], &gpu, &cpu)
                .is_err()
        );
        assert_eq!(gpu.peak(), 0);
        assert_eq!(cpu.peak(), 0);
    }
    let source = vec![2.5; 208];
    for phase in 0..5 {
        let cancelled_cpu = rrrah_core::MemoryBudget::new(4096);
        let cancelled_gpu = rrrah_core::MemoryBudget::new(8192);
        let mut checks = 0;
        let final_check = rrrah_gpu::RgbePlan::new([16, 13], [3, 0, 2, 1], &device.limits())
            .unwrap()
            .dispatches
            .len()
            + 6;
        assert!(matches!(
            compute.execute_managed_with_cancel(
                &queue,
                &source,
                [16, 13],
                [3, 0, 2, 1],
                &cancelled_gpu,
                &cancelled_cpu,
                || {
                    checks += 1;
                    match phase {
                        0 => true,
                        1 => cancelled_gpu.used() > 0,
                        2 => checks == final_check,
                        3 => checks == final_check - 1,
                        _ => checks == 5,
                    }
                },
            ),
            Err(rrrah_gpu::RgbeComputeError::Cancelled)
        ));
        assert_eq!(cancelled_cpu.used(), 0);
        assert_eq!(cancelled_gpu.used(), 0);
    }

    let zero = rrrah_core::MemoryBudget::new(0);
    assert!(
        compute
            .execute_managed(&queue, &source, [16, 13], [3, 0, 2, 1], &zero, &cpu)
            .is_err()
    );
    assert_eq!(cpu.used(), 0);
    assert!(
        compute
            .execute_managed(&queue, &source, [16, 13], [3, 0, 2, 1], &gpu, &zero)
            .is_err()
    );
    assert_eq!(gpu.used(), 0);
    assert_eq!(gpu.peak(), 0, "CPU refusal precedes GPU reservation");
    let result = compute
        .execute_managed(&queue, &source, [16, 13], [3, 0, 2, 1], &gpu, &cpu)
        .unwrap();
    assert!(result.iter().all(|p| *p == [2.5; 4]));
    assert_eq!(cpu.used(), 3328);
    assert_eq!(gpu.used(), 0);
    assert_eq!(gpu.peak(), 7680);
    let owner = result.clone();
    drop(result);
    assert_eq!(cpu.used(), 3328);
    drop(owner);
    assert_eq!(cpu.used(), 0);
}

#[test]
#[ignore = "requires independent DSC-F828 full-sensor dump; authored four-channel normalization, not as-shot color qualification"]
fn full_f828_managed_rgbe_matches_cpu_reference() {
    let bytes = std::fs::read(std::env::var("RRRAH_SRF_ORACLE").unwrap()).unwrap();
    assert_eq!(bytes.len(), 3360 * 2460 * 2);
    let quad = [3u32, 0, 2, 1];
    let cpu = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
    let mut source = cpu.try_buffer(3360 * 2460, 0f32).unwrap();
    // Deliberately authored gains exercise all planes. They do not infer E WB.
    let gains = [1.2f32, 0.8, 1.4, 0.9];
    for (i, (value, pair)) in source.iter_mut().zip(bytes.chunks_exact(2)).enumerate() {
        let x = i % 3360;
        let y = i / 3360;
        let plane = quad[(y % 2) * 2 + x % 2] as usize;
        *value = f32::from(u16::from_le_bytes(pair.try_into().unwrap())) / 16368. * gains[plane];
    }
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("full RGBE adapter: {:?}", adapter.get_info());
    for binding_limit in [
        wgpu::Limits::default().max_storage_buffer_binding_size,
        1024 * 1024,
    ] {
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: binding_limit,
            ..Default::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: limits,
            ..Default::default()
        }))
        .unwrap();
        let plan = rrrah_gpu::RgbePlan::new([3360, 2460], quad, &device.limits()).unwrap();
        if binding_limit == 1024 * 1024 {
            assert!(plan.source_bytes > u64::from(binding_limit));
            assert!(plan.dispatches.iter().any(|d| d.source_offset > 0));
        }
        let gpu = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
        let compute = rrrah_gpu::RgbeCompute::new(&device);
        let mut elapsed_ms = Vec::new();
        let mut last = None;
        for run in 0..3 {
            let started = std::time::Instant::now();
            let output = compute
                .execute_managed(&queue, &source, [3360, 2460], quad, &gpu, &cpu)
                .unwrap();
            elapsed_ms.push(started.elapsed().as_secs_f64() * 1000.);
            assert_eq!(gpu.used(), 0);
            if run == 2 {
                last = Some(output);
            } else {
                drop(output);
            }
        }
        let actual = last.unwrap();
        elapsed_ms.sort_by(f64::total_cmp);
        eprintln!(
            "full RGBE managed timing: binding={binding_limit}; runs=3; min_ms={:.3}; p50_ms={:.3}; max_ms={:.3}; includes validation/upload/execute/readback; excludes pipeline/source preparation",
            elapsed_ms[0], elapsed_ms[1], elapsed_ms[2]
        );
        for (i, pixel) in actual.iter().enumerate() {
            let expected = rrrah_core::rgbe::interpolate_planes(
                quad,
                [(i % 3360) as u32, (i / 3360) as u32],
                [3360, 2460],
                |x, y| source[(y * 3360 + x) as usize] as f64,
            )
            .unwrap();
            for (a, b) in pixel.iter().zip(expected) {
                assert!((*a as f64 - b).abs() < 1e-5, "pixel {i}");
            }
        }
        assert_eq!(gpu.used(), 0);
        assert_eq!(gpu.peak(), plan.gpu_buffer_bytes);
        assert_eq!(cpu.used(), 165312000);
        drop(actual);
        assert_eq!(cpu.used(), 33062400);
        eprintln!(
            "full RGBE binding window: limit={binding_limit}; dispatches={}; all channel values equal; GPU credit zero",
            plan.dispatches.len()
        );
    }
    drop(source);
    assert_eq!(cpu.used(), 0);
    eprintln!("full RGBE: 33,062,400 channel values equal; CPU/GPU final usage zero");
}

#[test]
fn cancelled_large_rgbe_validation_does_not_admit_output_resources() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let compute = rrrah_gpu::RgbeCompute::new(&device);
    let source = vec![2.5; 128 * 65];
    let cpu = rrrah_core::MemoryBudget::new(0);
    let gpu = rrrah_core::MemoryBudget::new(0);
    let mut checkpoints = 0;
    let result =
        compute.execute_managed_with_cancel(&queue, &source, [128, 65], [3, 0, 2, 1], &gpu, &cpu, || {
            checkpoints += 1;
            checkpoints == 3
        });
    assert!(matches!(result, Err(rrrah_gpu::RgbeComputeError::Cancelled)));
    assert_eq!(checkpoints, 3);
    assert_eq!(cpu.peak(), 0);
    assert_eq!(gpu.peak(), 0);
}

#[test]
fn odd_black_repeat_grid_normalization_to_managed_metal_compute() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("RGBE grid compute adapter: {:?}", adapter.get_info());
    for (height, binding_limit, site_count) in [(13, 1024, 221_u64), (33, 1024, 561_u64)] {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits {
                max_storage_buffer_binding_size: binding_limit,
                ..Default::default()
            },
            ..Default::default()
        }))
        .unwrap();
        let mut mosaic = common::profiled_pattern_mosaic(
            17,
            height,
            1000.,
            [2., 3., 4., 5.],
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
            |x, y| {
                let black = ((y % 5) * 3 + x % 3) * 20;
                ((1000 + black) / 2) as u16
            },
        );
        use rrrah_core::CfaColor::{Blue, Emerald, Green, Red};
        mosaic.metadata.cfa = Some(rrrah_core::CfaPattern {
            width: 2,
            height: 2,
            cells: vec![Emerald, Red, Blue, Green],
        });
        mosaic.metadata.black_level = rrrah_core::LevelGrid {
            width: 3,
            height: 5,
            components: 1,
            values: (0..15).map(|i| (i * 20) as f32).collect(),
        };
        let cpu = rrrah_core::MemoryBudget::new(65536);
        let gpu = rrrah_core::MemoryBudget::new(65536);
        let (calibration, normalized) =
            rrrah_core::rgbe::Calibration::normalize_managed(&mosaic, &cpu, &|| false).unwrap();
        assert_eq!(cpu.used(), site_count * 4 + 60);
        for (i, sample) in normalized.iter().enumerate() {
            let phase = ((i / 17 % 2) * 2 + i % 17 % 2) as usize;
            assert_eq!(*sample, [2.5, 1., 2., 1.5][phase]);
        }
        let plan =
            rrrah_gpu::RgbePlan::new(calibration.dimensions(), calibration.quad(), &device.limits()).unwrap();
        assert!(plan.dispatches.len() > 1);
        let compute = rrrah_gpu::RgbeCompute::new(&device);
        let result = compute
            .execute_managed(
                &queue,
                &normalized,
                calibration.dimensions(),
                calibration.quad(),
                &gpu,
                &cpu,
            )
            .unwrap();
        // Constant independent calibrated R/G/B/E values, including odd edges and
        // dispatch boundaries; no interpolation implementation used as the oracle.
        for pixel in result.iter() {
            assert_eq!(*pixel, [1., 1.5, 2., 2.5]);
        }
        assert_eq!(gpu.used(), 0);
        assert_eq!(cpu.used(), site_count * 20 + 60);
        drop(normalized);
        assert_eq!(cpu.used(), site_count * 16 + 60);
        let retained = result.clone();
        drop(result);
        assert_eq!(cpu.used(), site_count * 16 + 60);
        drop(retained);
        assert_eq!(cpu.used(), 60);
        drop(calibration);
        assert_eq!(cpu.used(), 0);
        eprintln!(
            "RGBE odd grid compute: 3x5 grid; 17x{height} sensor; four independent planes exact; split dispatch and odd edges; CPU/GPU credit zero"
        );
    }
}

#[test]
fn signed_f32_extremes_interpolate_finitely_against_f64_reference() {
    let Some(gpu_device) = common::qualification_gpu() else {
        return;
    };
    let dimensions = [5u32, 5];
    let quad = [3usize, 0, 2, 1];
    let source: Vec<f32> = (0..25)
        .map(|i| {
            if ((i % 5) / 2 + (i / 5) / 2) % 2 == 0 {
                f32::MAX
            } else {
                -f32::MAX
            }
        })
        .collect();
    let compute = rrrah_gpu::RgbeCompute::new(&gpu_device.device);
    let gpu = rrrah_core::MemoryBudget::new(8192);
    let cpu = rrrah_core::MemoryBudget::new(25 * 16);
    let output = compute
        .execute_managed(
            &gpu_device.queue,
            &source,
            dimensions,
            quad.map(|v| v as u32),
            &gpu,
            &cpu,
        )
        .unwrap();
    let axis = |coordinate: usize, phase: usize| {
        let last = phase + ((4 - phase) / 2) * 2;
        let lo = (phase + coordinate.saturating_sub(phase) / 2 * 2).min(last);
        let hi = (lo + 2).min(last);
        let weight = if hi == lo || coordinate < lo {
            0.
        } else {
            (coordinate - lo) as f64 / (hi - lo) as f64
        };
        (lo, hi, weight)
    };
    for y in 0..5 {
        for x in 0..5 {
            for phase in 0..4 {
                let (lx, hx, wx) = axis(x, phase % 2);
                let (ly, hy, wy) = axis(y, phase / 2);
                let a = f64::from(source[ly * 5 + lx]) * (1. - wx) + f64::from(source[ly * 5 + hx]) * wx;
                let b = f64::from(source[hy * 5 + lx]) * (1. - wx) + f64::from(source[hy * 5 + hx]) * wx;
                let expected = (a * (1. - wy) + b * wy) as f32;
                let actual = output[y * 5 + x][quad[phase]];
                assert!(actual.is_finite(), "({x},{y}) channel={phase}: {actual}");
                assert_eq!(actual, expected, "({x},{y}) channel={phase}");
            }
        }
    }
    drop(output);
    assert_eq!(cpu.used(), 0);
    assert_eq!(gpu.used(), 0);
}
