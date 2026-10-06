mod common;
#[test]
fn thumbnail_queue_refusal_validation_slot_reuse_and_completion() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("GPU required");
    eprintln!("filmstrip queue adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let budget = rrrah_core::MemoryBudget::new(256);
    let mut strip = rrrah_gpu::FilmstripRenderer::new(&device, common::READBACK_FORMAT, [640.0, 480.0])
        .with_upload_queue_budget(budget.clone());
    for (w, h, data) in [(0, 1, vec![]), (1, 1, vec![0; 3]), (u32::MAX, u32::MAX, vec![])] {
        assert!(matches!(
            strip.try_upload_tile(&device, &queue, w, h, &data),
            Err(rrrah_gpu::FilmstripUploadError::Invalid)
        ));
    }
    assert_eq!(budget.peak(), 0);
    let pressure = budget.try_reserve(256).unwrap();
    assert!(matches!(
        strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]),
        Err(rrrah_gpu::FilmstripUploadError::Memory(_))
    ));
    drop(pressure);
    let first = strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        strip.try_upload_tile(&device, &queue, 65, 1, &vec![255; 260]),
        Err(rrrah_gpu::FilmstripUploadError::Memory(_))
    ));
    strip.remove_tile(first);
    let second = strip.try_upload_tile(&device, &queue, 1, 1, &[128; 4]).unwrap();
    assert_eq!(first, second);
    drop(strip);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
    assert_eq!(budget.peak(), 256);
}

#[test]
fn thumbnail_texture_credit_competes_with_raster_and_releases_on_slot_removal() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("thumbnail residency adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let parent = rrrah_core::MemoryBudget::new(16);
    let mut raster =
        rrrah_gpu::RasterRenderer::new_with_budget(&device, common::READBACK_FORMAT, parent.child(16));
    let source = rrrah_core::DecodedRaster::new(
        1,
        1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(vec![1.0; 4]).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    raster.upload(&device, &queue, &source).unwrap();
    let mut strip = rrrah_gpu::FilmstripRenderer::new(&device, common::READBACK_FORMAT, [640.0, 480.0])
        .with_texture_budget(parent.child(8));
    assert!(matches!(
        strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]),
        Err(rrrah_gpu::FilmstripUploadError::TextureMemory(_))
    ));
    assert_eq!(strip.resident_bytes(), 0);
    raster.clear_image();
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let first = strip.try_upload_tile(&device, &queue, 1, 1, &[255; 4]).unwrap();
    let second = strip.try_upload_tile(&device, &queue, 1, 1, &[128; 4]).unwrap();
    assert_eq!(strip.resident_bytes(), 8);
    assert!(matches!(
        strip.try_upload_tile(&device, &queue, 1, 1, &[64; 4]),
        Err(rrrah_gpu::FilmstripUploadError::TextureMemory(_))
    ));
    assert_eq!(parent.used(), 8);
    strip.remove_tile(first);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(parent.used(), 4);
    strip.remove_tile(first); // Duplicate removals never release another slot's credit.
    assert_eq!(parent.used(), 4);
    assert_eq!(
        strip.try_upload_tile(&device, &queue, 1, 1, &[64; 4]).unwrap(),
        first
    );
    strip.remove_tile(second);
    drop(strip);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(parent.used(), 0);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}
