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

#[test]
fn hdr_filmstrip_thumbnail_is_linear_and_releases_resources() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &wgpu::RequestAdapterOptions::default())).unwrap();
    eprintln!("HDR filmstrip adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let textures = rrrah_core::MemoryBudget::new(4);
    let uploads = rrrah_core::MemoryBudget::new(256);
    let mut strip = rrrah_gpu::FilmstripRenderer::new(&device, wgpu::TextureFormat::Rgba16Float, [128., 128.])
        .with_texture_budget(textures.clone()).with_upload_queue_budget(uploads.clone());
    let id = strip.try_upload_tile(&device, &queue, 1, 1, &[128, 128, 128, 255]).unwrap();
    strip.update(&device, &queue, &[rrrah_gpu::FilmstripTile { x: 0., texture: Some(id), label: String::new(), highlighted: false }]);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HDR filmstrip target"), size: wgpu::Extent3d { width: 128, height: 128, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("HDR filmstrip readback"), size: 256, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            ..Default::default()
        });
    }
    strip.encode(&mut encoder, &view);
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d { x: 20, y: 40, z: 0 }, aspect: wgpu::TextureAspect::All }, wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(1) } }, wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 });
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    for channel in 0..3 {
        let bits = u16::from_le_bytes([mapped[channel * 2], mapped[channel * 2 + 1]]);
        let exponent = i32::from((bits >> 10) & 31) - 15;
        let actual = (1. + f32::from(bits & 1023) / 1024.) * 2f32.powi(exponent);
        let expected = ((128f32 / 255. + 0.055) / 1.055).powf(2.4);
        assert!((actual - expected).abs() < 0.0005, "HDR thumbnail must be linear: {actual} vs {expected}");
    }
    assert_eq!(u16::from_le_bytes([mapped[6], mapped[7]]), 0x3c00);
    drop(mapped);
    buffer.unmap();
    strip.remove_tile(id);
    drop(strip);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!((textures.used(), uploads.used()), (0, 0));
    assert!(pollster::block_on(error_scope.pop()).is_none());
}
