mod common;

#[test]
fn raster_replacement_accounts_for_old_texture_and_queued_upload() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance, &wgpu::RequestAdapterOptions::default(),
    )).expect("actual GPU required");
    eprintln!("replacement adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor::default(),
    )).unwrap();
    let budget = rrrah_core::MemoryBudget::new(32);
    let mut renderer = rrrah_gpu::RasterRenderer::new_with_budget(
        &device, common::READBACK_FORMAT, budget.clone(),
    );
    let frame = |red| rrrah_core::DecodedRaster::new(1, 1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(vec![red, 0.0, 0.0, 1.0]).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    ).unwrap();
    let complete = || {
        let submission = queue.submit([]);
        device.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None }).unwrap();
    };
    renderer.upload(&device, &queue, &frame(1.0)).unwrap();
    complete();
    assert_eq!(budget.used(), 16);
    let pressure = budget.try_reserve(1).unwrap();
    assert!(renderer.upload(&device, &queue, &frame(2.0)).is_err());
    assert_eq!(budget.used(), 17);
    drop(pressure);
    let old_frame = renderer.resource_lease();
    renderer.upload(&device, &queue, &frame(2.0)).unwrap();
    complete();
    assert_eq!(budget.used(), 32);
    assert!(renderer.upload(&device, &queue, &frame(4.0)).is_err());
    drop(old_frame);
    assert_eq!(budget.used(), 16);
    renderer.upload(&device, &queue, &frame(4.0)).unwrap();
    complete();
    assert_eq!(budget.used(), 16);
    assert_eq!(budget.peak(), 32);
    drop(renderer);
    assert_eq!(budget.used(), 0);
}

#[test]
fn submitted_resources_keep_credit_after_renderer_destruction_until_completion() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("resource lease adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
    let mut raw = rrrah_gpu::RawRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    raw.upload_mosaic_with_tiling(
        &device,
        &queue,
        &common::uniform_mosaic(32, 32, 32768, 65535.0),
        rrrah_gpu::TilingOverrides {
            tile_size: Some(32),
            tile_halo: Some(2),
        },
    )
    .unwrap();
    let raw_bytes = raw.resident_bytes();
    let mut raster =
        rrrah_gpu::RasterRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    let image = rrrah_core::DecodedRaster::new(
        1,
        1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(vec![1.0; 4]).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    raster.upload(&device, &queue, &image).unwrap();
    let mut model =
        rrrah_gpu::ModelRenderer::new_with_budget(&device, common::READBACK_FORMAT, budget.clone());
    let triangles = [[[-1f32, -1., 0.], [1., -1., 0.], [0., 1., 0.]]];
    model.upload(&device, triangles.into_iter()).unwrap();
    model.resize(&device, [64, 64]).unwrap();
    let mut strip = rrrah_gpu::FilmstripRenderer::new(&device, common::READBACK_FORMAT, [64., 64.])
        .with_texture_budget(budget.clone());
    strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
    let expected = raw_bytes + 16 + 36 + 64 * 64 * 4 + 4;
    assert_eq!(budget.used(), expected);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("resource lease qualification"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: common::READBACK_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    raw.encode(&mut encoder, &view);
    raster.encode(&mut encoder, &view);
    model.encode(&mut encoder, &view);
    strip.encode(&mut encoder, &view);
    let mut lease = raw.resource_lease();
    lease.extend(raster.resource_lease());
    lease.extend(model.resource_lease());
    lease.extend(strip.resource_lease());
    drop((raw, raster, model, strip));
    assert_eq!(budget.used(), expected);
    assert!(budget.try_reserve(budget.limit()).is_err());
    queue.submit([encoder.finish()]);
    lease.retain_until_complete(&queue);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
    assert!(pollster::block_on(errors.pop()).is_none());
}

#[test]
fn texture_upload_holds_credit_without_frame_or_explicit_resource_snapshot() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(4);
    let mut strip = rrrah_gpu::FilmstripRenderer::new(&device, common::READBACK_FORMAT, [64., 64.])
        .with_texture_budget(budget.clone());
    let tile = strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
    strip.remove_tile(tile);
    drop(strip);
    assert_eq!(budget.used(), 4);
    assert!(budget.try_reserve(1).is_err());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
}
