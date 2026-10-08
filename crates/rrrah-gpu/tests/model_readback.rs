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
    let translated = source.map(|t| t.map(|p| [p[0] + 1e12, p[1] - 1e12, p[2] + 1e12]));
    assert_eq!(
        reference.pixels,
        gpu.render_model(&translated, [64, 64], 0., 0.).pixels
    );
    for scale in [1e300, f64::from_bits(1), f64::MAX] {
        let scaled = source.map(|t| t.map(|p| p.map(|v| v * scale)));
        assert_eq!(
            reference.pixels,
            gpu.render_model(&scaled, [64, 64], 0., 0.).pixels,
            "scale {scale}"
        );
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
