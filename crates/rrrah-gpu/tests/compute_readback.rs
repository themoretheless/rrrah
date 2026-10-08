mod common;
use wgpu::util::DeviceExt;

#[test]
fn exposure_readback_cancellation_releases_mapping_and_allows_retry() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("actual GPU required");
    eprintln!("cancelled readback adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let input = vec![[4.0, -2.0, 0.125, 0.375]; 8200];
    let bytes = input.len() as u64 * 16;
    let cpu = rrrah_core::MemoryBudget::new(bytes);
    let gpu = rrrah_core::MemoryBudget::new(bytes * 3 + 16);
    for flat in [false, true] {
        let mut admitted_polls = 0;
        let mut cancel = || {
            if gpu.used() > 0 {
                admitted_polls += 1;
            }
            // Admission, before submit, first copy chunk, then second chunk.
            admitted_polls == 4
        };
        let cancelled = if flat {
            matches!(
                compute.execute_interleaved_with_cancel(
                    &queue,
                    bytemuck::cast_slice(&input),
                    1.0,
                    &gpu,
                    &cpu,
                    &mut cancel,
                ),
                Err(rrrah_gpu::ExposureError::Cancelled)
            )
        } else {
            matches!(
                compute.execute_managed_with_cancel(&queue, &input, 1.0, &gpu, &cpu, &mut cancel,),
                Err(rrrah_gpu::ExposureError::Cancelled)
            )
        };
        assert!(cancelled);
        assert_eq!(admitted_polls, 4);
        assert_eq!(cpu.used(), 0);
        assert_eq!(gpu.used(), 0);
        let retry = compute.execute_managed(&queue, &input, 1.0, &gpu, &cpu).unwrap();
        assert!(retry.iter().all(|pixel| *pixel == [8.0, -4.0, 0.25, 0.375]));
        drop(retry);
        assert_eq!(cpu.used(), 0);
        assert_eq!(gpu.used(), 0);
    }
}

#[test]
fn interleaved_exposure_preserves_bits_and_managed_raster_ownership() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("actual GPU required");
    eprintln!("interleaved compute adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let input: Vec<f32> = (0..129)
        .flat_map(|i| [i as f32 / 8.0, -2.0, 0.125, i as f32 / 256.0])
        .collect();
    let bytes = input.len() as u64 * 4;
    let gpu_bytes = bytes * 3 + 3 * 16;
    let cpu = rrrah_core::MemoryBudget::new(bytes);
    let gpu = rrrah_core::MemoryBudget::new(gpu_bytes);
    assert!(matches!(
        compute
            .execute_interleaved_with_cancel(&queue, &input[..input.len() - 1], 1.0, &gpu, &cpu, || false,),
        Err(rrrah_gpu::ExposureError::Invalid(_))
    ));
    assert_eq!(cpu.peak(), 0);
    assert_eq!(gpu.peak(), 0);
    for (gpu_limit, cpu_limit) in [(gpu_bytes - 1, bytes), (gpu_bytes, bytes - 1)] {
        let short_gpu = rrrah_core::MemoryBudget::new(gpu_limit);
        let short_cpu = rrrah_core::MemoryBudget::new(cpu_limit);
        assert!(matches!(
            compute.execute_interleaved_with_cancel(&queue, &input, 1.0, &short_gpu, &short_cpu, || false,),
            Err(rrrah_gpu::ExposureError::Memory(_))
        ));
        assert_eq!(short_gpu.used(), 0);
        assert_eq!(short_cpu.used(), 0);
    }
    let output = compute
        .execute_interleaved_with_cancel(&queue, &input, 1.0, &gpu, &cpu, || false)
        .unwrap();
    for (index, (&actual, &source)) in output.iter().zip(&input).enumerate() {
        let expected = if index % 4 == 3 { source } else { source * 2.0 };
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(gpu.used(), 0);
    assert_eq!(gpu.peak(), gpu_bytes);
    let alias = output.clone();
    let pointer = output.as_ptr();
    let raster = rrrah_core::DecodedRaster::new(
        129,
        1,
        rrrah_core::RasterPixels::Rgba32Float(output),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(values) = raster.pixels() else {
        panic!()
    };
    assert_eq!(values.as_ptr(), pointer);
    drop(raster);
    assert_eq!(cpu.used(), bytes);
    drop(alias);
    assert_eq!(cpu.used(), 0);
    assert_eq!(cpu.peak(), bytes);
}
#[test]
fn linear_exposure_compute_preserves_hdr_alpha_and_dispatch_tail() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("compute adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..wgpu::Limits::default()
        },
        ..Default::default()
    }))
    .unwrap();
    // 129 pixels exceed the 64-pixel binding limit, forcing three real dispatches.
    let source_pixels: Vec<[f32; 4]> = (0..129).map(|i| [128.0 + i as f32, -2.0, 0.125, 0.25]).collect();
    let initial = vec![[9.0_f32; 4]; 130];
    let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&source_pixels),
        usage: wgpu::BufferUsages::STORAGE,
    });
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
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let finite_cpu = rrrah_core::MemoryBudget::new(16);
    let finite_gpu = rrrah_core::MemoryBudget::new(64);
    for pixel in [
        [f32::MAX, 0.0, 0.0, 1.0],
        [-f32::MAX, 0.0, 0.0, 1.0],
        [f32::INFINITY, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, f32::NAN],
    ] {
        assert!(matches!(
            compute.execute_managed(&queue, &[pixel], 1.0, &finite_gpu, &finite_cpu),
            Err(rrrah_gpu::ExposureError::Invalid(_))
        ));
        assert_eq!(finite_cpu.peak(), 0);
        assert_eq!(finite_gpu.peak(), 0);
    }
    let cancel_cpu = rrrah_core::MemoryBudget::new(16);
    let cancel_gpu = rrrah_core::MemoryBudget::new(64);
    assert!(matches!(
        compute.execute_managed_with_cancel(&queue, &[[1.0; 4]], 0.0, &cancel_gpu, &cancel_cpu, || true,),
        Err(rrrah_gpu::ExposureError::Cancelled)
    ));
    assert_eq!(cancel_cpu.peak(), 0);
    assert_eq!(cancel_gpu.peak(), 0);
    assert!(matches!(
        compute.execute_managed_with_cancel(
            &queue,
            &[[1.0; 4]],
            0.0,
            &cancel_gpu,
            &cancel_cpu,
            || cancel_gpu.used() > 0,
        ),
        Err(rrrah_gpu::ExposureError::Cancelled)
    ));
    assert_eq!(cancel_cpu.used(), 0);
    assert_eq!(cancel_gpu.used(), 0);
    // Cancellation observed only after the submitted readback must also release
    // output and in-flight reservations, and the next request must remain usable.
    let mut checks = 0;
    assert!(matches!(
        compute.execute_managed_with_cancel(&queue, &[[1.0; 4]], 0.0, &cancel_gpu, &cancel_cpu, || {
            checks += 1;
            checks == 6
        },),
        Err(rrrah_gpu::ExposureError::Cancelled)
    ));
    assert_eq!(checks, 6);
    assert_eq!(cancel_cpu.used(), 0);
    assert_eq!(cancel_gpu.used(), 0);
    let validation_pixels = vec![[1.0; 4]; 8193];
    let validation_cpu = rrrah_core::MemoryBudget::new(8193 * 16);
    let validation_gpu = rrrah_core::MemoryBudget::new(8193 * 48 + 4096);
    let mut validation_checks = 0;
    assert!(matches!(
        compute.execute_managed_with_cancel(
            &queue,
            &validation_pixels,
            0.0,
            &validation_gpu,
            &validation_cpu,
            || {
                validation_checks += 1;
                validation_checks == 3
            },
        ),
        Err(rrrah_gpu::ExposureError::Cancelled)
    ));
    assert_eq!(validation_cpu.peak(), 0);
    assert_eq!(validation_gpu.peak(), 0);
    let maximum = compute
        .execute_managed(
            &queue,
            &[[f32::MAX, -f32::MAX, 0.0, 1.0]],
            0.0,
            &finite_gpu,
            &finite_cpu,
        )
        .unwrap();
    assert_eq!(maximum[0], [f32::MAX, -f32::MAX, 0.0, 1.0]);
    drop(maximum);
    assert_eq!(finite_cpu.used(), 0);
    assert_eq!(finite_gpu.used(), 0);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(
        compute
            .encode(&mut encoder, &source, &output, 129, f32::NAN)
            .is_err()
    );
    assert!(
        compute
            .encode(&mut encoder, &source, &source.clone(), 129, 1.0)
            .is_err()
    );
    assert!(compute.encode(&mut encoder, &source, &output, 130, 1.0).is_err());
    compute.encode(&mut encoder, &source, &output, 129, 1.0).unwrap();
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
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .expect("mapped compute readback");
    let pixels: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
    for (i, pixel) in pixels[..129].iter().enumerate() {
        assert_eq!(pixel, &[256.0 + 2.0 * i as f32, -4.0, 0.25, 0.25]);
    }
    assert_eq!(pixels[129], [9.0; 4]);
    drop(mapped);
    readback.unmap();
    let cpu = rrrah_core::MemoryBudget::new(129 * 16);
    // Three 64-pixel-or-smaller dispatches need three 16-byte uniforms.
    let required_gpu = 129 * 16 * 3 + 3 * 16;
    let gpu = rrrah_core::MemoryBudget::new(required_gpu);
    let tight_gpu = rrrah_core::MemoryBudget::new(required_gpu - 1);
    assert!(matches!(
        compute.execute_managed(&queue, &source_pixels, 1.0, &tight_gpu, &cpu),
        Err(rrrah_gpu::ExposureError::Memory(_))
    ));
    assert_eq!(tight_gpu.used(), 0);
    assert_eq!(cpu.used(), 0);
    // Simulate another live GPU owner. Admission must roll back CPU credit,
    // preserve the other owner's credit, then recover on the same budgets.
    let competing_gpu = gpu.try_reserve(1).unwrap();
    assert!(matches!(
        compute.execute_managed(&queue, &source_pixels, 1.0, &gpu, &cpu),
        Err(rrrah_gpu::ExposureError::Memory(_))
    ));
    assert_eq!(cpu.used(), 0);
    assert_eq!(gpu.used(), 1);
    drop(competing_gpu);
    assert_eq!(gpu.used(), 0);
    let tight_cpu = rrrah_core::MemoryBudget::new(129 * 16 - 1);
    assert!(matches!(
        compute.execute_managed(&queue, &source_pixels, 1.0, &gpu, &tight_cpu),
        Err(rrrah_gpu::ExposureError::Memory(_))
    ));
    assert_eq!(gpu.peak(), 1, "CPU refusal preserves the prior GPU peak");
    let managed = compute
        .execute_managed(&queue, &source_pixels, 1.0, &gpu, &cpu)
        .unwrap();
    for (index, pixel) in managed.iter().enumerate() {
        assert_eq!(pixel, &[256.0 + 2.0 * index as f32, -4.0, 0.25, 0.25]);
    }
    assert!(managed.is_managed());
    assert_eq!(gpu.peak(), required_gpu);
    assert_eq!(gpu.used(), 0);
    assert_eq!(cpu.used(), 129 * 16);
    let shared = managed.clone();
    drop(managed);
    assert_eq!(cpu.used(), 129 * 16);
    drop(shared);
    assert_eq!(cpu.used(), 0);
    assert!(
        compute
            .execute_managed(&queue, &source_pixels, f32::NAN, &gpu, &cpu)
            .is_err()
    );
    assert_eq!(cpu.used(), 0);
    let parent = rrrah_core::MemoryBudget::new(required_gpu + 129 * 16 - 1);
    assert!(matches!(
        compute.execute_managed(
            &queue,
            &source_pixels,
            1.0,
            &parent.child(required_gpu),
            &parent.child(129 * 16)
        ),
        Err(rrrah_gpu::ExposureError::Memory(_))
    ));
    assert_eq!(parent.used(), 0, "shared-parent refusal rolls back CPU admission");
    let empty_budget = rrrah_core::MemoryBudget::new(0);
    assert!(
        compute
            .execute_managed(&queue, &[], 0.0, &empty_budget, &empty_budget)
            .unwrap()
            .is_empty()
    );
    assert_eq!(empty_budget.peak(), 0);
}

#[test]
#[ignore = "full-resolution managed compute uses about 2 GB of tracked buffers"]
fn full_sensor_managed_exposure_budgets_and_checks_every_pixel() {
    let count = 6188_u32 * 4120;
    let bytes = u64::from(count) * 16;
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!(
        "managed full-resolution compute adapter: {:?}",
        adapter.get_info()
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_buffer_size: bytes,
            ..wgpu::Limits::default()
        },
        ..Default::default()
    }))
    .unwrap();
    let cpu = rrrah_core::MemoryBudget::new(bytes * 2);
    let limits = device.limits();
    let alignment = u64::from(limits.min_storage_buffer_offset_alignment).max(16) / 16;
    let maximum = (u64::from(limits.max_storage_buffer_binding_size) / 16)
        .min(u64::from(limits.max_compute_workgroups_per_dimension) * 64);
    let chunk = maximum / alignment * alignment;
    let expected_gpu = bytes * 3 + u64::from(count).div_ceil(chunk) * 16;
    let gpu = rrrah_core::MemoryBudget::new(expected_gpu);
    let mut pixels = cpu.try_buffer(count as usize, [0.0_f32; 4]).unwrap();
    for (index, pixel) in pixels.iter_mut().enumerate() {
        *pixel = [4.0 + (index % 1024) as f32 / 128.0, -0.5, 0.25, 0.75];
    }
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let flat_started = std::time::Instant::now();
    let flat = compute
        .execute_interleaved_with_cancel(&queue, bytemuck::cast_slice(&pixels), -1.0, &gpu, &cpu, || false)
        .unwrap();
    let flat_elapsed_ms = flat_started.elapsed().as_secs_f64() * 1000.0;
    for (index, pixel) in flat.chunks_exact(4).enumerate() {
        assert_eq!(
            pixel,
            [2.0 + (index % 1024) as f32 / 256.0, -0.25, 0.125, 0.75],
            "interleaved full sensor pixel {index}"
        );
    }
    assert_eq!(gpu.used(), 0);
    drop(flat);
    assert_eq!(cpu.used(), bytes);
    eprintln!(
        "interleaved full-resolution exposure: pixels={count}, end_to_end_ms={flat_elapsed_ms:.3}, all pixels exact"
    );
    let started = std::time::Instant::now();
    let result = compute
        .execute_managed(&queue, &pixels, -1.0, &gpu, &cpu)
        .unwrap();
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    for (index, pixel) in result.iter().enumerate() {
        assert_eq!(
            *pixel,
            [2.0 + (index % 1024) as f32 / 256.0, -0.25, 0.125, 0.75],
            "managed full sensor pixel {index}"
        );
    }
    assert_eq!(gpu.peak(), expected_gpu);
    assert_eq!(gpu.used(), 0);
    assert_eq!(cpu.peak(), bytes * 2);
    drop(pixels);
    assert_eq!(cpu.used(), bytes);
    let shared = result.clone();
    drop(result);
    assert_eq!(cpu.used(), bytes);
    drop(shared);
    assert_eq!(cpu.used(), 0);
    eprintln!(
        "managed full-resolution exposure: pixels={count}, end_to_end_ms={elapsed_ms:.3}, gpu_peak={}, cpu_peak={}, all pixels exact, final used=0",
        gpu.peak(),
        cpu.peak()
    );
}

#[test]
#[ignore = "allocates about 0.8 GB of GPU buffers; full-resolution local qualification"]
fn full_sensor_linear_exposure_chunked_readback() {
    let count = 6188_u32 * 4120;
    let bytes = u64::from(count) * 16;
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("full-resolution compute adapter: {:?}", adapter.get_info());
    assert!(adapter.limits().max_buffer_size >= bytes);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_buffer_size: bytes,
            ..wgpu::Limits::default()
        },
        ..Default::default()
    }))
    .unwrap();
    let pixels: Vec<[f32; 4]> = (0..count)
        .map(|i| [4.0 + (i % 1024) as f32 / 128.0, -0.5, 0.25, 0.75])
        .collect();
    let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("full sensor compute source"),
        contents: bytemuck::cast_slice(&pixels),
        usage: wgpu::BufferUsages::STORAGE,
    });
    drop(pixels);
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("full sensor compute output"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 48,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    // Complete any creation/upload work before measuring the compute submission.
    let setup = queue.submit([]);
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(setup),
            timeout: None,
        })
        .unwrap();
    let mut times = Vec::new();
    for iteration in 0..8 {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let started = std::time::Instant::now();
        compute
            .encode(&mut encoder, &source, &output, count, -1.0)
            .unwrap();
        let submission = queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();
        if iteration >= 3 {
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    for (i, pixel) in [0_u32, count / 2, count - 1].into_iter().enumerate() {
        encoder.copy_buffer_to_buffer(&output, u64::from(pixel) * 16, &readback, i as u64 * 16, 16);
    }
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
    for (value, index) in actual.iter().zip([0_u32, count / 2, count - 1]) {
        assert_eq!(*value, [2.0 + (index % 1024) as f32 / 256.0, -0.25, 0.125, 0.75]);
    }
    drop(mapped);
    readback.unmap();
    times.sort_by(f64::total_cmp);
    eprintln!(
        "full-resolution compute: {count} pixels, {bytes} bytes per buffer, samples_ms={times:?}, p50_ms={:.3}, max_ms={:.3}",
        times[2], times[4]
    );
}

#[test]
fn resident_exposure_preserves_hdr_tail_and_budgeted_gpu_ownership() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("actual GPU required");
    eprintln!("resident exposure adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let compute = rrrah_gpu::LinearExposureCompute::new(&device);
    let input = vec![[4., -2., 0.125, 0.375]; 8200];
    let bytes = input.len() as u64 * 16;
    let root = rrrah_core::MemoryBudget::new(bytes * 2 + 16);
    for checkpoint in 1..=6 {
        let mut polls = 0;
        assert!(matches!(
            compute.execute_resident(&queue, &input, 1., &root, || {
                polls += 1;
                polls == checkpoint
            }),
            Err(rrrah_gpu::ExposureError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
    }
    let refused = rrrah_core::MemoryBudget::new(bytes * 2 + 15);
    assert!(
        compute
            .execute_resident(&queue, &input, 1., &refused, || false)
            .is_err()
    );
    assert_eq!(refused.used(), 0);
    let output = compute
        .execute_resident(&queue, &input, 1., &root, || false)
        .unwrap();
    assert_eq!(output.pixels(), 8200);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test-only resident exposure oracle readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(output.buffer(), 0, &readback, 0, bytes);
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    let values: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
    assert_eq!(values.len(), 8200);
    assert!(values.iter().all(|p| *p == [8., -4., 0.25, 0.375]));
    drop(mapped);
    readback.unmap();
    assert_eq!(root.used(), bytes);
    let held = output.clone();
    drop(output);
    assert_eq!(root.used(), bytes);
    let downstream = held.resource_lease();
    drop(held);
    assert_eq!(root.used(), bytes);
    drop(downstream);
    assert_eq!(root.used(), 0);
    let abandoned = compute
        .execute_resident(&queue, &input, 1., &root, || false)
        .unwrap();
    drop(abandoned);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(root.used(), 0);
}
