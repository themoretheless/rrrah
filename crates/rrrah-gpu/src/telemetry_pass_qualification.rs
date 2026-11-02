//! Real HUD clear/load pass hypothesis. Not image rendering or display FPS.
use super::*;
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.008,
    g: 0.012,
    b: 0.02,
    a: 1.0,
};
fn encode(
    hud: &HudRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    fused: bool,
    queries: Option<&wgpu::QuerySet>,
) {
    let stamps = |begin, end| {
        queries.map(|query_set| wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(end),
        })
    };
    if fused {
        if queries.is_none() {
            hud.encode_with_clear(encoder, view, CLEAR);
        } else {
            hud.encode_pass(encoder, view, wgpu::LoadOp::Clear(CLEAR), stamps(0, 3));
        }
    } else {
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("telemetry original clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: stamps(0, 1),
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        hud.encode_pass(encoder, view, wgpu::LoadOp::Load, stamps(2, 3));
    }
}
fn percentile(v: &mut [u128], p: usize) -> u128 {
    v.sort_unstable();
    v[(v.len() * p).div_ceil(100) - 1]
}
#[test]
#[cfg(target_os = "macos")]
#[ignore = "real Metal telemetry pass readback, resize, native allocation and timestamp A/A A/B"]
fn telemetry_clear_pass_pairs_metal() {
    use objc2_metal::{MTLCopyAllDevices, MTLDevice};
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("actual Metal");
    eprintln!("TELEMETRY_ADAPTER {:?}", adapter.get_info());
    assert!(adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY));
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .unwrap();
    let devices = MTLCopyAllDevices();
    assert_eq!(devices.len(), 1);
    let native = devices.objectAtIndex(0);
    assert_eq!(native.name().to_string(), adapter.get_info().name);
    let initial = native.currentAllocatedSize();
    let challenge = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 8 * 1024 * 1024,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    challenge.get_mapped_range_mut(..).unwrap().slice(..).fill(1);
    challenge.unmap();
    assert!(native.currentAllocatedSize() >= initial + 8 * 1024 * 1024);
    drop(challenge);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: 4,
    });
    let resolved = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let timestamps = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 32,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut hud = HudRenderer::new(&device, format, [640., 480.]);
    for [width, height] in [[640, 480], [1248, 900], [320, 240]] {
        hud.resize(&queue, [width as f32, height as f32]);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let row = (width * 4).div_ceil(256) * 256;
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for empty in [true, false] {
            let cards = if empty {
                Vec::new()
            } else {
                (0..12)
                    .map(|_| {
                        HudCard::new("DECODE / 123 ms", "12.34 ms", "measured: synthetic test stage")
                            .with_status("MEASURED")
                    })
                    .collect()
            };
            hud.update_cards(&device, &queue, &cards);
            let mut reference = None;
            for fused in [false, true] {
                let mut encoder = device.create_command_encoder(&Default::default());
                encode(&hud, &mut encoder, &view, fused, None);
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &target,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &read,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row),
                            rows_per_image: Some(height),
                        },
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                read.slice(..)
                    .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let bytes = read.slice(..).get_mapped_range().unwrap().to_vec();
                read.unmap();
                if let Some(reference) = reference.as_ref() {
                    assert_eq!(&bytes, reference, "{width}x{height} empty={empty}");
                } else {
                    reference = Some(bytes);
                }
            }
        }
        if width != 1248 || std::env::var_os("RRRAH_TELEMETRY_CORRECTNESS_ONLY").is_some() {
            continue;
        }
        for control in [true, false] {
            for pair in 0..13 {
                for mode in [pair % 2, 1 - pair % 2] {
                    let mut cpu = Vec::new();
                    let mut gpu = Vec::new();
                    let mut wall = Vec::new();
                    let mut peak = 0;
                    for _ in 0..30 {
                        let start = std::time::Instant::now();
                        let mut encoder = device.create_command_encoder(&Default::default());
                        encode(&hud, &mut encoder, &view, control || mode == 1, Some(&queries));
                        queue.submit([encoder.finish()]);
                        cpu.push(start.elapsed().as_nanos());
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        wall.push(start.elapsed().as_nanos());
                        // Metal query resolve must happen after completion in a separate submit.
                        let mut encoder = device.create_command_encoder(&Default::default());
                        encoder.resolve_query_set(&queries, 0..4, &resolved, 0);
                        encoder.copy_buffer_to_buffer(&resolved, 0, &timestamps, 0, 32);
                        queue.submit([encoder.finish()]);
                        let (tx, rx) = std::sync::mpsc::channel();
                        timestamps
                            .slice(..)
                            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
                        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        rx.recv().unwrap().unwrap();
                        let mapped = timestamps.slice(..).get_mapped_range().unwrap();
                        let begin = u64::from_le_bytes(mapped[..8].try_into().unwrap());
                        let end = u64::from_le_bytes(mapped[24..32].try_into().unwrap());
                        assert!(end > begin);
                        gpu.push(((end - begin) as f64 * f64::from(queue.get_timestamp_period())) as u128);
                        drop(mapped);
                        timestamps.unmap();
                        peak = peak.max(native.currentAllocatedSize());
                    }
                    if pair >= 3 {
                        eprintln!(
                            "TELEMETRY_PAIR control={control} pair={} mode={mode} cpu_p95_ns={} gpu_p95_ns={} wall_p95_ns={} native_peak={peak}",
                            pair - 3,
                            percentile(&mut cpu, 95),
                            percentile(&mut gpu, 95),
                            percentile(&mut wall, 95)
                        );
                    }
                }
            }
        }
    }
    drop(hud);
    drop(timestamps);
    drop(resolved);
    drop(queries);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    eprintln!(
        "TELEMETRY_COMPLETE exact_empty_and_cards=true sizes=3 resources_dropped=true native_after={}",
        native.currentAllocatedSize()
    );
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "actual Metal editor panel contrast over black/white/extreme HDR backgrounds"]
fn editor_panels_do_not_inherit_extreme_hdr_background_metal() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    eprintln!("PANEL_CONTRAST_ADAPTER {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba16Float;
    let mut hud = HudRenderer::new(&device, format, [320., 400.]);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 320,
            height: 400,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 2560 * 400,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    for panel in [EditPanel::Conflict, EditPanel::Curve, EditPanel::Histogram] {
        hud.update_edit_panel(
            &device,
            &queue,
            panel,
            &rrrah_core::edit::EditState::default(),
            Some(&rrrah_core::histogram::Histogram::default()),
        );
        let mut reference: Option<Vec<u8>> = None;
        let mut outside_previous: Option<Vec<u8>> = None;
        for background in [-16., 0., 1., 16., 1024., 65504.] {
            let mut encoder = device.create_command_encoder(&Default::default());
            hud.encode_with_clear(
                &mut encoder,
                &target.create_view(&Default::default()),
                wgpu::Color {
                    r: background,
                    g: background,
                    b: background,
                    a: 1.,
                },
            );
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &read,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(2560),
                        rows_per_image: Some(400),
                    },
                },
                wgpu::Extent3d {
                    width: 320,
                    height: 400,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = read.slice(..).get_mapped_range().unwrap();
            let outside = mapped[4 * 2560 + 4 * 8..4 * 2560 + 5 * 8].to_vec();
            if let Some(previous) = outside_previous.replace(outside.clone()) {
                assert_ne!(
                    previous, outside,
                    "scene outside the panel must retain its new brightness"
                );
            }
            let bottom = if panel == EditPanel::Conflict { 146 } else { 374 };
            let pixels: Vec<u8> = (12..bottom)
                .flat_map(|y| mapped[y * 2560 + 12 * 8..y * 2560 + 304 * 8].iter().copied())
                .collect();
            if let Some(reference) = reference.as_ref() {
                assert!(
                    reference == &pixels,
                    "{panel:?} panel inherits scene background={background}; retained UI contrast is not bounded"
                );
            } else {
                reference = Some(pixels);
            }
            drop(mapped);
            read.unmap();
        }
        eprintln!("PANEL_CONTRAST panel={panel:?} backgrounds=6 exact_background_independence=true");
    }
    eprintln!(
        "PANEL_CONTRAST_COMPLETE cases=18 linear_half_float=true physical_HDR_display_not_qualified=true"
    );
}
