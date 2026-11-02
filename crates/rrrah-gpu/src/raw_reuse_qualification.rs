//! Rejected in-place atlas reuse: cancellation cannot restore a visible texture.
use super::*;
#[test]
#[cfg(target_os = "macos")]
#[ignore = "actual Metal negative control: in-place RAW atlas reuse corrupts retained frame"]
fn cancelled_in_place_reuse_changes_the_retained_frame_metal() {
    use rrrah_core::{CfaColor, CfaPattern, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel};
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("actual Metal");
    eprintln!("RAW_REUSE_ADAPTER {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let metadata = RawMetadata {
        make: "synthetic".into(),
        model: "reuse-negative".into(),
        width: 64,
        height: 64,
        components_per_pixel: 1,
        bits_per_sample: 16,
        photometric: Photometric::Cfa,
        cfa: Some(CfaPattern {
            width: 2,
            height: 2,
            cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
        }),
        black_level: LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.],
        },
        white_level: WhiteLevel(vec![4095.]),
        white_balance: [1.; 4],
        xyz_to_camera: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0.; 3]],
        active_area: None,
        crop_area: None,
        orientation: Orientation::Normal,
    };
    let mosaic = DecodedMosaic::new(metadata, std::sync::Arc::new(vec![2048; 4096])).unwrap();
    for format in [
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Rgba16Float,
    ] {
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut renderer = RawRenderer::new_with_budget(&device, format, budget.clone());
        if format == wgpu::TextureFormat::Rgba16Float {
            renderer
                .set_output_mode(&queue, RawOutputMode::SceneLinear)
                .unwrap();
        }
        renderer.upload_mosaic(&device, &queue, &mosaic).unwrap();
        renderer.update_view(
            &queue,
            ViewParameters {
                viewport: [64.; 2],
                zoom: 2.,
                ..Default::default()
            },
        );
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row = if format == wgpu::TextureFormat::Rgba16Float {
            512
        } else {
            256
        };
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: row * 64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let pixels = |renderer: &RawRenderer| {
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.encode(&mut encoder, &target.create_view(&Default::default()));
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
                        bytes_per_row: Some(row as u32),
                        rows_per_image: Some(64),
                    },
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let result = read.slice(..).get_mapped_range().unwrap().to_vec();
            read.unmap();
            result
        };
        let before = pixels(&renderer);
        let resident = budget.used();
        let parameters = bytemuck::bytes_of(&renderer.parameters).to_vec();
        let texture = renderer.raw_texture.as_ref().unwrap();
        let extent = texture.size();
        assert_eq!(extent.depth_or_array_layers, 1);
        // Candidate reuses the live atlas, then receives cancellation before
        // recipe/bind-group publication. Retaining old uniforms cannot undo writes.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &vec![0; extent.width as usize * extent.height as usize * 2],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(extent.width * 2),
                rows_per_image: Some(extent.height),
            },
            extent,
        );
        let candidate_result: Result<(), GpuError> = Err(GpuError::Cancelled);
        assert!(candidate_result.is_err());
        assert_eq!(parameters, bytemuck::bytes_of(&renderer.parameters));
        assert_eq!(budget.used(), resident);
        let after = pixels(&renderer);
        assert_ne!(before, after, "unsafe candidate must fail retained-frame parity");
        eprintln!(
            "RAW_REUSE_NEGATIVE format={format:?} cancelled=true unchanged_uniforms=true unchanged_budget=true retained_pixels_changed=true"
        );
        drop(renderer);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
    }
    eprintln!("RAW_REUSE_COMPLETE cases=2 final_credits=0 production_candidate_not_enabled=true");
}
