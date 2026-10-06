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
