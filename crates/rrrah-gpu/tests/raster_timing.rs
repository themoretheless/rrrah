//! Hardware timestamp qualification; not decoder or upload timing.
mod common;
use std::sync::Arc;

#[test]
#[ignore = "manual actual-GPU timestamp comparison; 4096-square input / 2048-square HDR target"]
fn raster_single_layer_gpu_timestamp_timing() {
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(&instance, &Default::default())).unwrap();
    assert!(adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY));
    eprintln!("raster timestamp adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .unwrap();
    let source = rrrah_core::DecodedRaster::new(
        4096,
        4096,
        rrrah_core::RasterPixels::Rgba32Float(Arc::new([0.25, 0.5, 2., 0.75].repeat(4096 * 4096)).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let budget = rrrah_core::MemoryBudget::new(source.pixel_capacity_bytes());
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, budget.clone());
    renderer.upload(&device, &queue, &source).unwrap();
    renderer.update_view(
        &queue,
        rrrah_gpu::ViewParameters {
            viewport: [2048., 2048.],
            zoom: 2048. / 2016.,
            ..Default::default()
        },
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timestamp HDR target"),
        size: wgpu::Extent3d {
            width: 2048,
            height: 2048,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let resolved = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mut times = Vec::new();
    for iteration in 0..35 {
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.encode_with_timestamps(
            &mut encoder,
            &view,
            Some(wgpu::RenderPassTimestampWrites {
                query_set: &queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
        );
        // Metal end-of-pass timestamps become available after command-buffer completion.
        // Resolve in a second submission; resolving in the render submission returned end=0.
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&queries, 0..2, &resolved, 0);
        encoder.copy_buffer_to_buffer(&resolved, 0, &readback, 0, 16);
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let begin = u64::from_le_bytes(mapped[..8].try_into().unwrap());
        let end = u64::from_le_bytes(mapped[8..16].try_into().unwrap());
        assert!(
            end > begin,
            "monotone GPU timestamps required: begin={begin} end={end} period={}",
            queue.get_timestamp_period()
        );
        let ms = (end - begin) as f64 * f64::from(queue.get_timestamp_period()) / 1e6;
        assert!(ms.is_finite());
        drop(mapped);
        readback.unmap();
        if iteration >= 5 {
            times.push(ms);
        }
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "{{\"median_gpu_ms\":{},\"p95_gpu_ms\":{},\"max_gpu_ms\":{},\"warmups\":5,\"samples_gpu_ms\":{:?},\"source\":[4096,4096],\"target\":[2048,2048],\"target_format\":\"Rgba16Float\"}}",
        (sorted[14] + sorted[15]) / 2.,
        sorted[28],
        sorted[29],
        times
    );
    drop(renderer);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(budget.used(), 0);
}
