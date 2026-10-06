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
    for dimensions in [[2u32, 2], [3, 5], [17, 13], [16, 13]] {
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
            if dimensions[0] == 16 {
                limits.max_storage_buffer_binding_size = 1024;
            }
            let plan = rrrah_gpu::RgbePlan::new(dimensions, quad, &limits).unwrap();
            if dimensions[0] == 16 {
                assert_eq!(plan.dispatches.len(), 4);
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
                            resource: source.as_entire_binding(),
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
        let final_check = rrrah_gpu::RgbePlan::new([16, 13], [3, 0, 2, 1], &device.limits()).unwrap().dispatches.len() + 6;
        assert!(matches!(compute.execute_managed_with_cancel(
            &queue, &source, [16, 13], [3, 0, 2, 1], &cancelled_gpu, &cancelled_cpu,
            || { checks += 1; match phase {
                0 => true,
                1 => cancelled_gpu.used() > 0,
                2 => checks == final_check,
                3 => checks == final_check - 1,
                _ => checks == 5,
            } },
        ), Err(rrrah_gpu::RgbeComputeError::Cancelled)));
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
    assert_eq!(gpu.peak(), 7616);
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
    let gpu = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
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
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let compute = rrrah_gpu::RgbeCompute::new(&device);
    let actual = compute
        .execute_managed(&queue, &source, [3360, 2460], quad, &gpu, &cpu)
        .unwrap();
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
    assert_eq!(gpu.peak(), 297561632);
    assert_eq!(cpu.used(), 165312000);
    drop(actual);
    drop(source);
    assert_eq!(cpu.used(), 0);
    eprintln!("full RGBE: 33,062,400 channel values equal; CPU/GPU final usage zero");
}

#[test]
fn cancelled_large_rgbe_validation_does_not_admit_output_resources() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance, &wgpu::RequestAdapterOptions::default(),
    )).unwrap();
    let (device, queue) = pollster::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor::default()),
    ).unwrap();
    let compute = rrrah_gpu::RgbeCompute::new(&device);
    let source = vec![2.5; 128 * 65];
    let cpu = rrrah_core::MemoryBudget::new(0);
    let gpu = rrrah_core::MemoryBudget::new(0);
    let mut checkpoints = 0;
    let result = compute.execute_managed_with_cancel(
        &queue, &source, [128, 65], [3, 0, 2, 1], &gpu, &cpu,
        || { checkpoints += 1; checkpoints == 3 },
    );
    assert!(matches!(result, Err(rrrah_gpu::RgbeComputeError::Cancelled)));
    assert_eq!(checkpoints, 3);
    assert_eq!(cpu.peak(), 0);
    assert_eq!(gpu.peak(), 0);
}
