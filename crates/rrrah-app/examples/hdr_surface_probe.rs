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
    attempts: u32,
}
struct Gpu {
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: rrrah_gpu::RasterRenderer,
    validation: wgpu::ErrorScopeGuard,
}
impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
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
        if !matches!(event, WindowEvent::RedrawRequested) {
            return;
        }
        let window = self.window.as_ref().unwrap().clone();
        if let Some(gpu) = &self.gpu {
            self.attempts += 1;
            match gpu.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    let view = frame.texture.create_view(&Default::default());
                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    gpu.renderer.encode(&mut encoder, &view);
                    gpu.queue.submit([encoder.finish()]);
                    window.pre_present_notify();
                    gpu.queue.present(frame);
                    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    println!(
                        "display after present: {:?}",
                        gpu.surface.display_hdr_info(&gpu.adapter)
                    );
                    let gpu = self.gpu.take().unwrap();
                    let error = pollster::block_on(gpu.validation.pop());
                    if let Some(error) = error {
                        self.error = Some(error.to_string());
                    } else {
                        println!("presented: RGBA16Float / ExtendedSrgbLinear; no validation errors");
                    }
                    event_loop.exit();
                }
                other => {
                    if self.attempts >= 120 {
                        self.error = Some(format!("surface frame unavailable after redraws: {other:?}"));
                        let gpu = self.gpu.take().unwrap();
                        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                        let validation = pollster::block_on(gpu.validation.pop());
                        println!("configuration validation: {validation:?}; no frame presented");
                        event_loop.exit();
                    } else {
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
                Ok::<_, anyhow::Error>(Gpu {
                    surface,
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
                event_loop.exit();
            }
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
fn main() -> anyhow::Result<()> {
    let events = EventLoop::new()?;
    let mut probe = Probe::default();
    events.run_app(&mut probe)?;
    if let Some(error) = probe.error {
        anyhow::bail!(error);
    }
    Ok(())
}
