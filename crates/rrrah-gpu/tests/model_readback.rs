mod common;

#[test]
fn model_has_coverage_orbits_and_fits_extreme_coordinates() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("model adapter: {}", gpu.adapter_name());
    let triangles = [[[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    let image = gpu.render_model(&triangles, [64, 64], 0., 0.);
    assert!(image.center()[0] > 100, "{:?}", image.center());
    assert!(image.pixel(0, 0)[0] < 40);
    let edge = gpu.render_model(&triangles, [64, 64], std::f32::consts::FRAC_PI_2, 0.);
    assert!(edge.center()[0] < 40);
    let huge = triangles.map(|triangle| triangle.map(|p| p.map(|v| v * f32::MAX)));
    let huge_image = gpu.render_model(&huge, [64, 64], 0., 0.);
    assert_eq!(image.pixels, huge_image.pixels);
    let empty = gpu.render_model(&[] as &[[[f32; 3]; 3]], [64, 64], 0., 0.);
    assert!(empty.max_channel_deviation([36, 36, 36, 255]) <= 1);
}

#[test]
fn depth_resolves_overlapping_facets_independent_of_submission_order() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    // Distinct slopes produce distinct illumination, making hidden-surface
    // failure visible rather than comparing identically shaded coplanar facets.
    let near = [[-1., -1., 0.6], [1., -1., 0.6], [0., 1., 0.6]];
    let far = [[-1., -1., -0.9], [1., -1., -0.3], [0., 1., -0.6]];
    let a = gpu.render_model(&[near, far], [64, 64], 0., 0.);
    let b = gpu.render_model(&[far, near], [64, 64], 0., 0.);
    assert_eq!(a.pixels, b.pixels);
    assert!(a.center()[0] > 100);
}

#[test]
fn float64_translation_and_scale_preserve_small_geometry() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let source = [[[-1_f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    let reference = gpu.render_model(&source, [64, 64], 0., 0.);
    let reference_hdr = gpu.render_model_half(&source, [64, 64], 0., 0.);
    assert!(reference_hdr.iter().all(|v| v & 0x7c00 != 0x7c00));
    let translated = source.map(|t| t.map(|p| [p[0] + 1e12, p[1] - 1e12, p[2] + 1e12]));
    assert_eq!(
        reference.pixels,
        gpu.render_model(&translated, [64, 64], 0., 0.).pixels
    );
    assert_eq!(reference_hdr, gpu.render_model_half(&translated, [64, 64], 0., 0.));
    for scale in [1e300, f64::from_bits(1), f64::MAX] {
        let scaled = source.map(|t| t.map(|p| p.map(|v| v * scale)));
        assert_eq!(
            reference.pixels,
            gpu.render_model(&scaled, [64, 64], 0., 0.).pixels,
            "scale {scale}"
        );
        assert_eq!(reference_hdr, gpu.render_model_half(&scaled, [64, 64], 0., 0.),
            "HDR scale {scale}");
    }
}

#[test]
fn model_budget_admits_vertices_and_depth_transactionally() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("model budget adapter: {:?}", adapter.get_info());
    let (device, _) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(100);
    let mut renderer =
        rrrah_gpu::ModelRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    let triangles = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    renderer.upload(&device, triangles.into_iter()).unwrap();
    renderer.resize(&device, [4, 4]).unwrap();
    assert_eq!(budget.used(), 100);
    assert_eq!(renderer.resident_bytes(), 100);
    assert!(matches!(
        renderer.upload(&device, triangles.into_iter()),
        Err(rrrah_gpu::ModelUploadError::Memory(_))
    ));
    assert!(matches!(
        renderer.resize(&device, [2, 2]),
        Err(rrrah_gpu::ModelUploadError::Memory(_))
    ));
    assert_eq!(renderer.resident_bytes(), 100);
    renderer.resize(&device, [4, 4]).unwrap(); // reuse requires no admission
    renderer.clear_model();
    assert_eq!(budget.used(), 64); // depth remains cached
    renderer.resize(&device, [2, 2]).unwrap();
    assert_eq!(budget.used(), 16);
    renderer.upload(&device, triangles.into_iter()).unwrap();
    assert_eq!(budget.used(), 52);
    drop(renderer);
    assert_eq!(budget.used(), 0);
}

#[test]
fn raster_and_model_resources_share_parent_admission() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let parent = rrrah_core::MemoryBudget::new(68);
    let mut raster =
        rrrah_gpu::RasterRenderer::new_with_budget(&device, common::READBACK_FORMAT, parent.child(32));
    let mut model =
        rrrah_gpu::ModelRenderer::new_with_budget(&device, common::READBACK_FORMAT, parent.child(52));
    let image = rrrah_core::DecodedRaster::new(
        1,
        1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(vec![1.0f32; 4]).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let triangles = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    raster.upload(&device, &queue, &image).unwrap();
    model.upload(&device, triangles.into_iter()).unwrap();
    model.resize(&device, [2, 2]).unwrap();
    assert_eq!(parent.used(), 68);
    assert!(matches!(
        raster.upload(&device, &queue, &image),
        Err(rrrah_gpu::RasterUploadError::Memory(_))
    ));
    assert_eq!(parent.used(), 68);
    assert!(raster.has_image());
    raster.clear_image();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(parent.used(), 52);
    // Parent has room, but model's local cap still refuses replacement overlap.
    assert!(matches!(
        model.resize(&device, [1, 1]),
        Err(rrrah_gpu::ModelUploadError::Memory(_))
    ));
    assert_eq!(parent.used(), 52);
    model.clear_model();
    model.resize(&device, [1, 1]).unwrap();
    assert_eq!(parent.used(), 4);
    model.upload(&device, triangles.into_iter()).unwrap();
    assert_eq!(parent.used(), 40);
    raster.upload(&device, &queue, &image).unwrap();
    assert_eq!(parent.used(), 56);
    assert_eq!(parent.peak(), 68);
    drop(model);
    drop(raster);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(parent.used(), 0);
}

#[test]
fn model_linear_hdr_target_matches_analytic_material_and_releases_budget() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(65536);
    let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&device, wgpu::TextureFormat::Rgba16Float, budget.clone());
    renderer.upload(&device, [[[-1f32, -1., 0.], [1., -1., 0.], [0., 1., 0.]]].into_iter()).unwrap();
    renderer.resize(&device, [64, 64]).unwrap();
    renderer.update_view(&queue, 1., 0., 0., 1.);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("model HDR target"), size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 64 * 64 * 8,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(512), rows_per_image: Some(64) } },
        wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 });
    let submitted = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device.poll(wgpu::PollType::Wait { submission_index: Some(submitted), timeout: None }).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    let pixel = |index: usize| -> [f32; 4] { std::array::from_fn(|channel| {
        let offset = index * 8 + channel * 2;
        let bits = u16::from_le_bytes([mapped[offset], mapped[offset + 1]]);
        let exponent = ((bits >> 10) & 31) as i32;
        let mantissa = (bits & 1023) as f32;
        assert_eq!(bits & 0x8000, 0, "expected nonnegative model channel");
        assert!(exponent < 31, "nonfinite model channel");
        if exponent == 0 { mantissa * 2f32.powi(-24) }
        else { (1. + mantissa / 1024.) * 2f32.powi(exponent - 15) }
    }) };
    let center = pixel(32 * 64 + 32);
    let light = 0.2 + 0.8 / 1.34f32.sqrt();
    for (actual, material) in center[..3].iter().zip([0.45, 0.55, 0.65]) {
        assert!((actual - material * light).abs() < 0.001, "linear material changed: {center:?}");
    }
    assert_eq!(center[3], 1.);
    let background = pixel(0);
    for channel in &background[..3] { assert!((channel - 0.018).abs() < 0.0001); }
    assert_eq!(background[3], 1.);
    drop(mapped);
    buffer.unmap();
    drop(renderer);
    assert_eq!(budget.used(), 0);
}

#[test]
fn cancelled_model_upload_preserves_resident_geometry_and_credit() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    let (device, _) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(72);
    let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    let triangles = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    renderer.upload(&device, triangles.into_iter()).unwrap();
    for checkpoint in 1..=5 {
        let mut checks = 0;
        assert!(matches!(renderer.upload_with_cancel(&device, triangles.into_iter(), || {
            checks += 1;
            checks == checkpoint
        }), Err(rrrah_gpu::ModelUploadError::Cancelled)));
        assert_eq!(checks, checkpoint);
        assert_eq!((budget.used(), renderer.resident_bytes()), (36, 36));
        renderer.upload(&device, triangles.into_iter()).unwrap();
        assert_eq!((budget.used(), renderer.resident_bytes()), (36, 36));
    }
    drop(renderer);
    assert_eq!(budget.used(), 0);
}

#[test]
fn cancelled_model_replacement_keeps_sdr_and_hdr_pixels_exact() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let size = [63, 65];
    let original = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    let replacement = vec![[[-1f64, -1., 0.], [1., -1., 0.], [-1., 1., 0.]]; 2049];
    for format in [common::READBACK_FORMAT, wgpu::TextureFormat::Rgba16Float] {
        let budget = rrrah_core::MemoryBudget::new(63 * 65 * 4 + 36 + 2049 * 36);
        let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&gpu.device, format, budget.clone());
        renderer.upload(&gpu.device, original.into_iter()).unwrap();
        renderer.resize(&gpu.device, size).unwrap();
        renderer.update_view(&gpu.queue, 63. / 65., 0., 0., 1.);
        let read = |renderer: &rrrah_gpu::ModelRenderer| -> Vec<u16> {
            if format == wgpu::TextureFormat::Rgba16Float {
                gpu.read_half_frame(size, |encoder, target| renderer.encode(encoder, target))
            } else {
                gpu.read_frame(size, |encoder, target| renderer.encode(encoder, target))
                    .pixels.into_iter().map(u16::from).collect()
            }
        };
        let expected = read(&renderer);
        let retained = budget.used();
        let foreign_instance = common::headless_instance();
        let foreign_adapter = pollster::block_on(common::request_adapter(&foreign_instance, &Default::default())).unwrap();
        let (foreign_device, _) = pollster::block_on(foreign_adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let foreign = rrrah_gpu::ModelRenderer::new_with_budget(&foreign_device, format, budget.clone());
        let wrong_owner = foreign.upload_preparer().prepare_with_cancel(
            replacement.iter().copied(), || false,
        ).unwrap();
        assert!(matches!(renderer.install_prepared_upload(wrong_owner),
            Err(rrrah_gpu::ModelUploadError::RendererMismatch)));
        assert_eq!(budget.used(), retained);
        assert_eq!(read(&renderer), expected);
        drop(foreign);
        let other_budget = rrrah_core::MemoryBudget::new(2049 * 36);
        for source_budget in [None, Some(&other_budget)] {
            let incompatible = rrrah_gpu::ModelRenderer::prepare_upload_with_cancel(
                &gpu.device, replacement.iter().copied(), source_budget, || false,
            ).unwrap();
            assert!(matches!(renderer.install_prepared_upload(incompatible),
                Err(rrrah_gpu::ModelUploadError::BudgetMismatch)));
            assert_eq!(other_budget.used(), 0);
            assert_eq!(read(&renderer), expected);
        }
        let worker_preparer = renderer.upload_preparer();
        let worker_geometry = replacement.clone();
        let obsolete = std::thread::spawn(move || {
            worker_preparer.prepare_with_cancel(worker_geometry.into_iter(), || false).unwrap()
        }).join().unwrap();
        assert_eq!(budget.used(), retained + 2049 * 36);
        assert_eq!(read(&renderer), expected);
        drop(obsolete);
        assert_eq!(budget.used(), retained);
        assert_eq!(read(&renderer), expected);
        for checkpoint in 1..=9 {
            let mut checks = 0;
            assert!(matches!(renderer.upload_with_cancel(&gpu.device, replacement.iter().copied(), || {
                checks += 1;
                checks == checkpoint
            }), Err(rrrah_gpu::ModelUploadError::Cancelled)));
            assert_eq!(checks, checkpoint);
            assert_eq!(budget.used(), retained);
            assert_eq!(read(&renderer), expected);
        }
        let pressure = budget.try_reserve(1).unwrap();
        assert!(matches!(renderer.upload(&gpu.device, replacement.iter().copied()),
            Err(rrrah_gpu::ModelUploadError::Memory(_))));
        assert_eq!(read(&renderer), expected);
        drop(pressure);
        let prepared = renderer.upload_preparer().prepare_with_cancel(
            replacement.iter().copied(), || false,
        ).unwrap();
        assert_eq!(read(&renderer), expected);
        renderer.install_prepared_upload(prepared).unwrap();
        assert_ne!(read(&renderer), expected);
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
    eprintln!("model replacement failures: SDR/HDR; nine cancellations and memory refusal retain exact frame; retry changes frame; final credit zero");
}

#[test]
fn prepared_model_replacement_keeps_submitted_resource_credit_until_completion() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let size = [63, 65];
    let depth_bytes = 63 * 65 * 4;
    let budget = rrrah_core::MemoryBudget::new(depth_bytes + 72);
    let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(
        &gpu.device, common::READBACK_FORMAT, budget.clone());
    let triangle = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    renderer.upload(&gpu.device, triangle.into_iter()).unwrap();
    renderer.resize(&gpu.device, size).unwrap();
    renderer.update_view(&gpu.queue, 63. / 65., 0., 0., 1.);
    let lease = renderer.resource_lease();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("old model submission after replacement"),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: common::READBACK_FORMAT, usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let errors = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    let commands = encoder.finish();
    let prepared = renderer.upload_preparer().prepare_with_cancel(triangle.into_iter(), || false).unwrap();
    assert_eq!(budget.used(), depth_bytes + 72);
    renderer.install_prepared_upload(prepared).unwrap();
    assert_eq!(budget.used(), depth_bytes + 72);
    drop(renderer);
    assert_eq!(budget.used(), depth_bytes + 36);
    let submission = gpu.queue.submit([commands]);
    lease.retain_until_complete(&gpu.queue);
    gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None }).unwrap();
    assert!(pollster::block_on(errors.pop()).is_none());
    assert_eq!(budget.used(), 0);
    eprintln!("prepared model lifetime: old encoded geometry/depth credit retained across replacement/drop; submission completed; final zero");
}

#[test]
fn refused_depth_resize_preserves_sdr_hdr_frame_and_retries() {
    let Some(gpu) = common::qualification_gpu() else { return; };
    let triangles = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    for format in [wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureFormat::Rgba16Float] {
        let budget = rrrah_core::MemoryBudget::new(36 + 63 * 65 * 4 + 67 * 69 * 4);
        let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&gpu.device, format, budget.clone());
        renderer.upload(&gpu.device, triangles.into_iter()).unwrap();
        renderer.resize(&gpu.device, [63, 65]).unwrap();
        renderer.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
        let read = |renderer: &rrrah_gpu::ModelRenderer, size| -> Vec<u16> {
            if format == wgpu::TextureFormat::Rgba16Float {
                gpu.read_half_frame(size, |e, t| renderer.encode(e, t))
            } else {
                gpu.read_frame(size, |e, t| renderer.encode(e, t)).pixels.into_iter().map(u16::from).collect()
            }
        };
        let before = read(&renderer, [63, 65]);
        let resident = budget.used();
        let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
        assert!(matches!(renderer.resize(&gpu.device, [67, 69]), Err(rrrah_gpu::ModelUploadError::Memory(_))));
        assert_eq!(budget.used(), budget.limit());
        assert_eq!(read(&renderer, [63, 65]), before);
        renderer.resize(&gpu.device, [63, 65]).unwrap();
        drop(pressure);
        assert_eq!(budget.used(), resident);
        renderer.resize(&gpu.device, [67, 69]).unwrap();
        renderer.update_view(&gpu.queue, 67. / 69., 0.4, -0.2, 1.);
        let after = read(&renderer, [67, 69]);
        let mut reference = rrrah_gpu::ModelRenderer::new(&gpu.device, format);
        reference.upload(&gpu.device, triangles.into_iter()).unwrap();
        reference.resize(&gpu.device, [67, 69]).unwrap();
        reference.update_view(&gpu.queue, 67. / 69., 0.4, -0.2, 1.);
        assert_eq!(after, read(&reference, [67, 69]));
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn model_depth_stays_on_renderer_device_with_foreign_resize_argument() {
    let Some(gpu) = common::qualification_gpu() else { return; };
    let foreign_instance = common::headless_instance();
    let foreign_adapter = pollster::block_on(common::request_adapter(&foreign_instance, &Default::default())).unwrap();
    let (foreign_device, _) = pollster::block_on(foreign_adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let triangles = [[[-1f64, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    for format in [wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureFormat::Rgba16Float] {
        let budget = rrrah_core::MemoryBudget::new(36 + 63 * 65 * 4);
        let mut renderer = rrrah_gpu::ModelRenderer::new_with_budget(&gpu.device, format, budget.clone());
        renderer.upload(&gpu.device, triangles.into_iter()).unwrap();
        renderer.resize(&foreign_device, [63, 65]).unwrap();
        renderer.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
        let mut reference = rrrah_gpu::ModelRenderer::new(&gpu.device, format);
        reference.upload(&gpu.device, triangles.into_iter()).unwrap();
        reference.resize(&gpu.device, [63, 65]).unwrap();
        reference.update_view(&gpu.queue, 63. / 65., 0.4, -0.2, 1.);
        if format == wgpu::TextureFormat::Rgba16Float {
            assert_eq!(gpu.read_half_frame([63, 65], |e, t| renderer.encode(e, t)),
                gpu.read_half_frame([63, 65], |e, t| reference.encode(e, t)));
        } else {
            assert_eq!(gpu.read_frame([63, 65], |e, t| renderer.encode(e, t)).pixels,
                gpu.read_frame([63, 65], |e, t| reference.encode(e, t)).pixels);
        }
        assert_eq!(budget.used(), budget.limit());
        drop(renderer);
        assert_eq!(budget.used(), 0);
    }
}
