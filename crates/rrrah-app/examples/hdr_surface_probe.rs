//! Main-thread, short-lived live-window HDR surface qualification. No physical
//! luminance/color measurement is inferred from successful presentation.
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
#[derive(Default)]
struct Probe {
    window: Option<Arc<Window>>,
    error: Option<String>,
    gpu: Option<Gpu>,
    unavailable_since: Option<std::time::Instant>,
    presented: u32,
    hold_seconds: u64,
    first_present: Option<std::time::Instant>,
    next_redraw: Option<std::time::Instant>,
    resize_probe: bool,
    resize_confirmed: bool,
    frames_after_resize: u32,
    finished: bool,
}
struct Gpu {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: ProbeRenderer,
    validation: wgpu::ErrorScopeGuard,
}
enum ProbeRenderer {
    Raster(rrrah_gpu::RasterRenderer),
    Raw(rrrah_gpu::RawRenderer),
    Model(rrrah_gpu::ModelRenderer),
}
impl ProbeRenderer {
    fn encode(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        match self {
            Self::Raster(renderer) => renderer.encode(encoder, view),
            Self::Raw(renderer) => renderer.encode(encoder, view),
            Self::Model(renderer) => renderer.encode(encoder, view),
        }
    }
}

impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Rrrah HDR surface qualification")
                        .with_inner_size(winit::dpi::LogicalSize::new(320., 160.)),
                )
                .expect("create qualification window"),
        );
        window.set_visible(true);
        window.focus_window();
        window.request_redraw();
        self.window = Some(window);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if self.finished {
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            self.error = Some("HDR surface probe closed before qualification completed".into());
            if let Some(gpu) = self.gpu.take() {
                if let Err(error) = gpu.device.poll(wgpu::PollType::wait_indefinitely()) {
                    self.error = Some(error.to_string());
                }
                if let Some(error) = pollster::block_on(gpu.validation.pop()) {
                    self.error = Some(error.to_string());
                }
            }
            self.finished = true;
            event_loop.exit();
            return;
        }
        if let WindowEvent::Resized(size) = event {
            if size.width > 0 && size.height > 0 {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.config.width = size.width;
                    gpu.config.height = size.height;
                    gpu.surface.configure(&gpu.device, &gpu.config);
                    let view = rrrah_gpu::ViewParameters {
                        viewport: [size.width as f32, size.height as f32],
                        ..Default::default()
                    };
                    match &mut gpu.renderer {
                        ProbeRenderer::Raster(renderer) => {
                            renderer.update_view(&gpu.queue, rrrah_gpu::ViewParameters { zoom: 40., ..view })
                        }
                        ProbeRenderer::Raw(renderer) => renderer.update_view(&gpu.queue, view),
                        ProbeRenderer::Model(renderer) => {
                            if let Err(error) = renderer.resize(&gpu.device, [size.width, size.height]) {
                                self.error = Some(error.to_string());
                                self.finished = true;
                                event_loop.exit();
                                return;
                            }
                            renderer.update_view(
                                &gpu.queue,
                                size.width as f32 / size.height as f32,
                                0.3,
                                0.2,
                                1.,
                            );
                        }
                    }
                    if self.resize_probe && size.width == 800 && size.height == 400 {
                        self.resize_confirmed = true;
                        println!("live resize confirmed: 800x400; HDR surface reconfigured");
                    }
                }
            }
            return;
        }
        if !matches!(event, WindowEvent::RedrawRequested) {
            return;
        }
        let window = self.window.as_ref().unwrap().clone();
        if let Some(gpu) = &self.gpu {
            match gpu.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    self.unavailable_since = None;
                    assert_eq!(
                        [frame.texture.width(), frame.texture.height()],
                        [gpu.config.width, gpu.config.height],
                        "surface dimensions must match configured viewport"
                    );
                    let view = frame.texture.create_view(&Default::default());
                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    gpu.renderer.encode(&mut encoder, &view);
                    gpu.queue.submit([encoder.finish()]);
                    window.pre_present_notify();
                    gpu.queue.present(frame);
                    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    self.presented += 1;
                    if self.resize_confirmed {
                        self.frames_after_resize += 1;
                    }
                    if self.resize_probe && self.presented == 30 {
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(800, 400));
                        println!("live resize requested: 800x400");
                    }
                    let first = self.first_present.get_or_insert_with(std::time::Instant::now);
                    if self.presented == 1 || self.presented % 30 == 0 {
                        println!(
                            "display after present: frames={} elapsed={:.3}s {:?}",
                            self.presented,
                            first.elapsed().as_secs_f64(),
                            gpu.surface.display_hdr_info(&gpu.adapter)
                        );
                    }
                    if first.elapsed() < std::time::Duration::from_secs(self.hold_seconds) {
                        self.next_redraw =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(16));
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                            std::time::Instant::now() + std::time::Duration::from_millis(16),
                        ));
                        return;
                    }
                    println!(
                        "presentation complete: frames={} elapsed={:.3}s",
                        self.presented,
                        first.elapsed().as_secs_f64()
                    );
                    if self.resize_probe {
                        if !self.resize_confirmed || self.frames_after_resize < 30 {
                            self.error = Some(format!(
                                "live resize incomplete: confirmed={} frames={}",
                                self.resize_confirmed, self.frames_after_resize
                            ));
                        }
                        println!(
                            "live resize presentation: confirmed={} frames_after_resize={}",
                            self.resize_confirmed, self.frames_after_resize
                        );
                    }
                    let gpu = self.gpu.take().unwrap();
                    let error = pollster::block_on(gpu.validation.pop());
                    if let Some(error) = error {
                        self.error = Some(error.to_string());
                    } else {
                        println!("presented: RGBA16Float / ExtendedSrgbLinear; no validation errors");
                    }
                    self.finished = true;
                    event_loop.exit();
                }
                other => {
                    // Redraw frequency varies with focus and window visibility. Bound
                    // consecutive unavailability by elapsed time, not total frames.
                    let unavailable = self
                        .unavailable_since
                        .get_or_insert_with(std::time::Instant::now)
                        .elapsed();
                    if unavailable >= std::time::Duration::from_secs(10) {
                        self.error = Some(format!(
                            "surface frame unavailable for {unavailable:?}: {other:?}"
                        ));
                        let gpu = self.gpu.take().unwrap();
                        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        let validation = pollster::block_on(gpu.validation.pop());
                        println!(
                            "configuration validation: {validation:?}; frames presented={}",
                            self.presented
                        );
                        self.finished = true;
                        event_loop.exit();
                    } else {
                        self.next_redraw =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(16));
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                            std::time::Instant::now() + std::time::Duration::from_millis(16),
                        ));
                    }
                }
            }
            return;
        }
        let result = (|| -> anyhow::Result<Gpu> {
            pollster::block_on(async {
                let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                    event_loop.owned_display_handle(),
                ));
                descriptor.backends = wgpu::Backends::METAL;
                let instance = wgpu::Instance::new(descriptor);
                let surface = instance.create_surface(window.clone())?;
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&surface),
                        ..Default::default()
                    })
                    .await?;
                println!("adapter: {:?}", adapter.get_info());
                println!(
                    "display before configure: {:?}",
                    surface.display_hdr_info(&adapter)
                );
                let caps = surface.get_capabilities(&adapter);
                println!("format/color capabilities: {:?}", caps.format_capabilities);
                anyhow::ensure!(
                    caps.format_capabilities
                        .iter()
                        .any(|v| v.format == wgpu::TextureFormat::Rgba16Float
                            && v.color_spaces
                                .contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR)),
                    "required linear HDR surface pair unavailable"
                );
                let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await?;
                let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let size = window.inner_size();
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: wgpu::TextureFormat::Rgba16Float,
                    width: size.width.max(1),
                    height: size.height.max(1),
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: caps.alpha_modes[0],
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                    color_space: wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
                };
                surface.configure(&device, &config);
                println!(
                    "display after configure: {:?}",
                    surface.display_hdr_info(&adapter)
                );
                let renderer = if let Some(path) = std::env::var_os("RRRAH_HDR_MODEL_SOURCE") {
                    anyhow::ensure!(
                        std::env::var_os("RRRAH_HDR_RAW_SOURCE").is_none(),
                        "choose exactly one RAW or model source"
                    );
                    let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(path));
                    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
                    request.memory_budget = Some(budget.clone());
                    let model = rrrah_decode::decode_model(&request)?;
                    println!(
                        "native model HDR surface: triangles={}; source={}",
                        model.triangle_count(),
                        request.path.display()
                    );
                    let mut renderer = rrrah_gpu::ModelRenderer::new(&device, config.format);
                    renderer.upload(&device, model.triangles())?;
                    renderer.resize(&device, [size.width.max(1), size.height.max(1)])?;
                    renderer.update_view(
                        &queue,
                        size.width.max(1) as f32 / size.height.max(1) as f32,
                        0.3,
                        0.2,
                        1.,
                    );
                    drop(model);
                    anyhow::ensure!(
                        budget.used() == 0,
                        "native model source credit retained after upload"
                    );
                    ProbeRenderer::Model(renderer)
                } else if let Some(path) = std::env::var_os("RRRAH_HDR_RAW_SOURCE") {
                    use rrrah_decode::RawDecoder;
                    let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(path));
                    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
                    request.memory_budget = Some(budget.clone());
                    let mosaic = rrrah_decode::NativeRawDecoder.decode(&request)?.mosaic;
                    println!(
                        "native RAW HDR surface: {}x{}; samples={}; source={}",
                        mosaic.metadata.width,
                        mosaic.metadata.height,
                        mosaic.pixels.len(),
                        request.path.display()
                    );
                    let mut renderer = rrrah_gpu::RawRenderer::new(&device, config.format);
                    renderer.set_output_mode(&queue, rrrah_gpu::RawOutputMode::SceneLinear)?;
                    renderer.upload_mosaic(&device, &queue, &mosaic)?;
                    renderer.update_view(
                        &queue,
                        rrrah_gpu::ViewParameters {
                            viewport: [size.width as f32, size.height as f32],
                            ..Default::default()
                        },
                    );
                    drop(mosaic);
                    anyhow::ensure!(
                        budget.used() == 0,
                        "native RAW source credit retained after upload"
                    );
                    ProbeRenderer::Raw(renderer)
                } else if let Some(path) = std::env::var_os("RRRAH_HDR_RASTER_SOURCE") {
                    let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(path));
                    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
                    request.memory_budget = Some(budget.clone());
                    let policy = match std::env::var("RRRAH_HDR_UNTAGGED_COLOR") {
                        Ok(policy) => policy,
                        Err(std::env::VarError::NotPresent) => "strict".to_owned(),
                        Err(error) => return Err(error.into()),
                    };
                    match policy.as_str() {
                        "strict" => {}
                        "srgb" => request.assume_untagged_srgb = true,
                        "linear-srgb" => request.assume_untagged_linear_srgb = true,
                        _ => anyhow::bail!("HDR untagged color must be strict, srgb or linear-srgb"),
                    }
                    println!("native raster untagged color policy: {policy}");
                    let decoded = rrrah_decode::decode_raster(&request)?;
                    let raster =
                        rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget))?;
                    println!(
                        "native raster HDR surface: {}x{}; pixel_aspect={:?}; source={}",
                        raster.width(),
                        raster.height(),
                        raster.pixel_aspect(),
                        request.path.display()
                    );
                    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr(&device);
                    renderer.upload(&device, &queue, &raster)?;
                    renderer.update_view(
                        &queue,
                        rrrah_gpu::ViewParameters {
                            viewport: [size.width as f32, size.height as f32],
                            zoom: 4.,
                            ..Default::default()
                        },
                    );
                    drop(raster);
                    drop(decoded);
                    anyhow::ensure!(
                        budget.used() == 0,
                        "native raster source credit retained after upload"
                    );
                    println!("native raster source credit after upload: 0");
                    ProbeRenderer::Raster(renderer)
                } else {
                    let pixels: Vec<f32> = [0.18, 1., 2., 4.]
                        .into_iter()
                        .flat_map(|v| [v, v, v, 1.])
                        .collect();
                    let raster = rrrah_core::DecodedRaster::new(
                        4,
                        1,
                        rrrah_core::RasterPixels::Rgba32Float(Arc::new(pixels).into()),
                        rrrah_core::RasterColorSpace::LinearSrgb,
                    )?;
                    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr(&device);
                    renderer.upload(&device, &queue, &raster)?;
                    renderer.update_view(
                        &queue,
                        rrrah_gpu::ViewParameters {
                            viewport: [size.width as f32, size.height as f32],
                            zoom: 40.,
                            ..Default::default()
                        },
                    );
                    ProbeRenderer::Raster(renderer)
                };
                Ok::<_, anyhow::Error>(Gpu {
                    surface,
                    config,
                    adapter,
                    device,
                    queue,
                    renderer,
                    validation,
                })
            })
        })();
        match result {
            Ok(gpu) => {
                self.gpu = Some(gpu);
                window.request_redraw();
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                self.finished = true;
                event_loop.exit();
            }
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        if self
            .next_redraw
            .is_none_or(|deadline| std::time::Instant::now() >= deadline)
        {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
fn main() -> anyhow::Result<()> {
    let selected = [
        "RRRAH_HDR_RASTER_SOURCE",
        "RRRAH_HDR_RAW_SOURCE",
        "RRRAH_HDR_MODEL_SOURCE",
    ]
    .into_iter()
    .filter(|name| std::env::var_os(name).is_some())
    .count();
    anyhow::ensure!(selected <= 1, "choose at most one raster, RAW or model source");
    let events = EventLoop::new()?;
    let hold_seconds = match std::env::var("RRRAH_HDR_SURFACE_SECONDS") {
        Ok(value) => value.parse::<u64>()?,
        Err(std::env::VarError::NotPresent) => 0,
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        hold_seconds <= 60,
        "HDR probe duration must be between 0 and 60 seconds"
    );
    let resize_probe = std::env::var("RRRAH_HDR_RESIZE_PROBE").is_ok_and(|v| v == "1");
    anyhow::ensure!(
        !resize_probe || hold_seconds >= 3,
        "resize probe requires at least3 seconds"
    );
    let mut probe = Probe {
        hold_seconds,
        resize_probe,
        ..Default::default()
    };
    events.run_app(&mut probe)?;
    if let Some(error) = probe.error {
        anyhow::bail!(error);
    }
    anyhow::ensure!(
        probe.finished && probe.presented > 0,
        "HDR surface probe exited before presentation and validation completed"
    );
    Ok(())
}
