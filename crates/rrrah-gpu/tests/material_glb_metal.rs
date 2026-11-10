//! Native glTF decoding feeding Forma's material renderer: GLB vertex streams and textures decoded
//! by rrrah-decode, uploaded and drawn through `gpu_kit` (re-exported by rrrah-gpu).
#![cfg(target_os = "macos")]

use rrrah_core::MemoryBudget;
use rrrah_gpu::{
    MaterialAlpha, MaterialDraw, MaterialParameters, MaterialRenderer, MaterialSampler,
    MaterialTextureUploader, MaterialVertex,
};

#[test]
#[ignore = "actual Metal draw of a decoded GLB material"]
fn native_glb_material_draws_through_shared_renderer() {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    assert_eq!(adapter.get_info().backend, wgpu::Backend::Metal);
    eprintln!("MATERIAL_DRAW_ADAPTER {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::FLOAT32_FILTERABLE,
        ..Default::default()
    }))
    .unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let uploader = MaterialTextureUploader::new(&device, &queue);
    let renderer = MaterialRenderer::new(uploader.clone(), wgpu::TextureFormat::Rgba8UnormSrgb).unwrap();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 3,
            height: 3,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: target.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 768,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let render = |draw: &MaterialDraw| {
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let view = target.create_view(&Default::default());
            let dv = depth.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &dv,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            renderer.encode(draw, &mut pass).unwrap();
        }
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
                    bytes_per_row: Some(256),
                    rows_per_image: Some(3),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        draw.resource_lease().retain_until_complete(&queue);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = read.slice(..).get_mapped_range().unwrap();
        let pixel: [u8; 4] = mapped[260..264].try_into().unwrap();
        drop(mapped);
        read.unmap();
        pixel
    };
    // Native GLB -> material-selected vertex streams -> PNG pixels -> draw.
    let mut request = rrrah_decode::DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/glb/material-controls.glb"),
    );
    request.memory_budget = Some(budget.clone());
    let document = rrrah_decode::glb::GlbDocument::read(&request).unwrap();
    let streams = document
        .primitive(0, 0, &request)
        .unwrap()
        .vertex_streams(&request)
        .unwrap();
    let material = streams.material();
    let image = rrrah_decode::decode_glb_texture(
        &document,
        0,
        rrrah_decode::glb::GlbTextureRole::BaseColor,
        &request,
    )
    .unwrap();
    let rrrah_decode::GltfTexturePixels::Rgba8(samples) = image.pixels() else {
        panic!("authored RGBA8 source")
    };
    let color = uploader
        .upload_rgba8(
            &budget,
            [image.width(), image.height()],
            samples,
            true,
            Default::default(),
            || false,
        )
        .unwrap();
    let data = uploader
        .upload_rgba8(
            &budget,
            [image.width(), image.height()],
            samples,
            false,
            Default::default(),
            || false,
        )
        .unwrap();
    let source_vertices: Vec<MaterialVertex> = (0..streams.triangle_count())
        .flat_map(|i| streams.triangle(i, &request).unwrap().unwrap())
        .map(|v| MaterialVertex {
            position: v.position.map(|v| v as f32),
            normal: v.normal.unwrap().map(|v| v as f32),
            tangent: v.tangent.unwrap().map(|v| v as f32),
            color: v.color.map(|v| v as f32),
            texcoords: v.texcoords.map(|uv| uv.unwrap().map(|v| v as f32)),
        })
        .collect();
    let mut source_p = MaterialParameters::default();
    source_p.clip_from_world = [
        [2., 0., 0., 0.],
        [0., 2., 0., 0.],
        [0., 0., 1., 0.],
        [-1., -1., 0.5, 1.],
    ];
    source_p.base_color = material.base_color.map(|v| v as f32);
    source_p.emissive = [
        material.emissive[0] as f32,
        material.emissive[1] as f32,
        material.emissive[2] as f32,
        0.,
    ];
    source_p.factors = [
        material.metallic as f32,
        material.roughness as f32,
        material.normal_texture.unwrap().scale as f32,
        material.occlusion_texture.unwrap().strength as f32,
    ];
    source_p.alpha = [material.alpha_cutoff as f32, 0., 1., 0.];
    source_p.eye[3] = 0.;
    source_p.light_direction[3] = 0.;
    let draw = renderer
        .prepare(
            &budget,
            &source_vertices,
            source_p,
            [&color, &data, &data, &data, &color],
            MaterialAlpha::Blend,
            material.double_sided,
            false,
            || false,
        )
        .unwrap();
    let pixel = render(&draw);
    let opacity = 0.5 * (128. / 255.) * 0.75;
    for (c, (code, factor)) in [(128., 0.4), (64., 0.2), (192., 0.1)].into_iter().enumerate() {
        let encoded: f64 = code / 255.;
        let linear = if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        };
        let v = linear * factor * opacity;
        let result = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1. / 2.4) - 0.055
        };
        assert!(
            pixel[c].abs_diff((result * 255.).round() as u8) <= 1,
            "native material pixel {pixel:?}"
        );
    }
    assert_eq!(pixel[3], 48);
    eprintln!("native GLB material -> PNG -> GPU frame: {pixel:?}");
    drop(draw);
    drop(color);
    drop(data);
    drop(image);
    drop(document);
    drop(request);
}

fn read_float_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    mip: u32,
) -> Vec<f32> {
    let width = (texture.width() >> mip).max(1);
    let height = (texture.height() >> mip).max(1);
    let row = (width * 16).div_ceil(256) * 256;
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: mip,
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
    let mapped = read.slice(..).get_mapped_range().unwrap();
    let mut out = Vec::new();
    for y in 0..height as usize {
        let bytes = &mapped[y * row as usize..y * row as usize + width as usize * 16];
        out.extend(
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes(b.try_into().unwrap())),
        );
    }
    drop(mapped);
    read.unmap();
    out
}

#[test]
#[ignore = "actual Metal readback of a decoded 16-bit PNG texture"]
fn native_png16_texture_keeps_every_code_through_shared_uploader() {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::FLOAT32_FILTERABLE,
        ..Default::default()
    }))
    .unwrap();
    let uploader = MaterialTextureUploader::new(&device, &queue);
    // Exercise the actual native PNG adapter and the shared root, not only
    // authored GPU input. Compare with independently authored LE16 oracle.
    for role in [
        rrrah_decode::glb::GlbTextureRole::Normal,
        rrrah_decode::glb::GlbTextureRole::BaseColor,
    ] {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new("embedded-PNG16");
        request.memory_budget = Some(budget.clone());
        let image = rrrah_decode::decode_gltf_texture_bytes(
            include_bytes!("../../../tests/fixtures/glb/texture-codec/color6-16.png"),
            rrrah_decode::glb::GlbImageMime::Png,
            role,
            &request,
        )
        .unwrap();
        let rrrah_decode::GltfTexturePixels::Rgba16(pixels) = image.pixels() else {
            panic!("16-bit source narrowed")
        };
        let candidate = uploader
            .upload_rgba16(
                &budget,
                [image.width(), image.height()],
                pixels,
                role.is_srgb(),
                MaterialSampler::default(),
                || false,
            )
            .unwrap();
        let actual = read_float_texture(&device, &queue, candidate.texture(), 0);
        let expected = include_bytes!("../../../tests/fixtures/glb/texture-codec/color6-16.png.rgba16le");
        for (i, (&sample, bytes)) in actual.iter().zip(expected.chunks_exact(2)).enumerate() {
            let v = f64::from(sample);
            let code = if role.is_srgb() && i % 4 != 3 {
                if v <= 0.0031308 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                }
            } else {
                v
            };
            assert_eq!(
                (code * 65535.).round() as u16,
                u16::from_le_bytes(bytes.try_into().unwrap())
            );
        }
        assert_eq!(actual.len() * 2, expected.len());
        drop(candidate);
        drop(image);
        drop(request);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(budget.used(), 0);
        eprintln!("native PNG16 -> Metal {role:?}: all original codes retained; shared root zero");
    }
}
