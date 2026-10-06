//! Linear HDR exposure without tone mapping; independent of display transfer.
use wgpu::util::DeviceExt;
#[derive(Debug, thiserror::Error)]
pub enum ExposureError {
    #[error("GPU exposure cancelled")]
    Cancelled,
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("GPU exposure readback failed: {0}")]
    Readback(String),
}
#[derive(Debug)]
pub struct LinearExposureCompute {
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
}
impl LinearExposureCompute {
    /// Blocking, explicitly budgeted exposure/readback. The caller owns the
    /// input's CPU accounting. GPU accounting covers buffer sizes (including
    /// readback and dispatch uniforms), excluding driver/pipeline overhead.
    pub fn execute_managed(
        &self,
        queue: &wgpu::Queue,
        source_pixels: &[[f32; 4]],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, ExposureError> {
        self.execute_managed_with_cancel(queue, source_pixels, stops, gpu_budget, cpu_budget, || false)
    }

    /// Cooperatively cancels CPU validation/admission and discards obsolete
    /// results. Submitted GPU work still completes before this blocking method
    /// returns; its reservations remain live through queue completion.
    pub fn execute_managed_with_cancel(
        &self,
        queue: &wgpu::Queue,
        source_pixels: &[[f32; 4]],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, ExposureError> {
        self.execute_into(
            queue,
            source_pixels,
            stops,
            gpu_budget,
            cpu_budget,
            [0.0; 4],
            1,
            &mut cancelled,
        )
    }

    /// Direct flat RGBA32F input/output for DecodedRaster storage. No temporary
    /// array-of-pixels allocation or flattened result copy is required.
    pub fn execute_interleaved_with_cancel(
        &self,
        queue: &wgpu::Queue,
        samples: &[f32],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<f32>, ExposureError> {
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        let (pixels, remainder) = samples.as_chunks::<4>();
        if !remainder.is_empty() {
            return Err(ExposureError::Invalid("incomplete interleaved RGBA32F pixel"));
        }
        self.execute_into(queue, pixels, stops, gpu_budget, cpu_budget, 0.0, 4, cancelled)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_into<T: bytemuck::Pod>(
        &self,
        queue: &wgpu::Queue,
        source_pixels: &[[f32; 4]],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        initial: T,
        components: usize,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<T>, ExposureError> {
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
            return Err(ExposureError::Invalid("exposure outside finite -32..32 range"));
        }
        let count = u32::try_from(source_pixels.len())
            .map_err(|_| ExposureError::Invalid("too many RGBA32F pixels"))?;
        let gain = stops.exp2();
        for (index, pixel) in source_pixels.iter().enumerate() {
            if index % 4096 == 0 && cancelled() {
                return Err(ExposureError::Cancelled);
            }
            if pixel.iter().any(|value| !value.is_finite()) {
                return Err(ExposureError::Invalid("RGBA32F samples must be finite"));
            }
            if pixel[..3].iter().any(|value| !(value * gain).is_finite()) {
                return Err(ExposureError::Invalid("exposed RGB exceeds finite f32 range"));
            }
        }
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        if count == 0 {
            return Ok(cpu_budget.try_buffer(0, initial)?.freeze().into());
        }
        let bytes = u64::from(count) * 16;
        let limits = self.device.limits();
        if bytes > limits.max_buffer_size {
            return Err(ExposureError::Invalid("RGBA32F buffer exceeds device limit"));
        }
        let alignment = u64::from(limits.min_storage_buffer_offset_alignment).max(16) / 16;
        let maximum = (u64::from(limits.max_storage_buffer_binding_size) / 16)
            .min(u64::from(limits.max_compute_workgroups_per_dimension) * 64);
        let chunk = maximum / alignment * alignment;
        if chunk == 0 {
            return Err(ExposureError::Invalid(
                "device cannot bind an aligned RGBA32F chunk",
            ));
        }
        let reserved_bytes = bytes * 3 + u64::from(count).div_ceil(chunk) * 16;
        let cpu_reservation = cpu_budget.try_reserve(bytes)?;
        let reservation = std::sync::Arc::new(gpu_budget.try_reserve(reserved_bytes)?);
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        let output_len = source_pixels
            .len()
            .checked_mul(components)
            .ok_or(ExposureError::Invalid("RGBA32F output length overflow"))?;
        let mut result = cpu_reservation.try_buffer(output_len, initial)?;
        let source = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("managed exposure source"),
            contents: bytemuck::cast_slice(source_pixels),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed exposure output"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed exposure readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode(&mut encoder, &source, &output, count, stops)
            .map_err(ExposureError::Invalid)?;
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        let submission = queue.submit([encoder.finish()]);
        // Even a failed poll must not release accounting for in-flight work.
        let in_flight = reservation.clone();
        queue.on_submitted_work_done(move || drop(in_flight));
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
            let _ = tx.send(status);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .map_err(|error| ExposureError::Readback(error.to_string()))?;
        rx.recv()
            .map_err(|error| ExposureError::Readback(error.to_string()))?
            .map_err(|error| ExposureError::Readback(error.to_string()))?;
        let mapped = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|error| ExposureError::Readback(error.to_string()))?;
        // Bound obsolete CPU work to 4096 pixels even for full-resolution
        // readback. Always release the mapping before returning cancellation.
        let copied = (|| {
            for (destination, source) in result
                .chunks_mut(4096 * components)
                .zip(bytemuck::cast_slice::<u8, T>(&mapped).chunks(4096 * components))
            {
                if cancelled() {
                    return Err(ExposureError::Cancelled);
                }
                destination.copy_from_slice(source);
            }
            Ok(())
        })();
        drop(mapped);
        readback.unmap();
        copied?;
        if cancelled() {
            return Err(ExposureError::Cancelled);
        }
        Ok(result.freeze().into())
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("linear HDR exposure"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/linear_exposure.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("linear HDR exposure"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            device: device.clone(),
            pipeline,
        }
    }
    /// Encodes exposure over separate RGBA32F STORAGE buffers. Alpha is unchanged.
    /// Uses aligned subrange bindings and bounded dispatches for large buffers.
    /// This does not submit work or perform HDR display/tone mapping.
    /// The caller is responsible for finite samples and RGB overflow admission;
    /// `execute_managed` validates CPU input before allocating or submitting.
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        output: &wgpu::Buffer,
        count: u32,
        stops: f32,
    ) -> Result<(), &'static str> {
        if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
            return Err("exposure outside finite -32..32 range");
        }
        if count == 0 {
            return Ok(());
        }
        let bytes = u64::from(count) * 16;
        let limits = self.device.limits();
        if source.size() < bytes || output.size() < bytes {
            return Err("RGBA32F buffer is too short");
        }
        // Binding offsets must satisfy both device alignment and RGBA32F stride.
        let alignment_pixels = u64::from(limits.min_storage_buffer_offset_alignment).max(16) / 16;
        let max_pixels = (u64::from(limits.max_storage_buffer_binding_size) / 16)
            .min(u64::from(limits.max_compute_workgroups_per_dimension) * 64);
        let chunk_pixels = max_pixels / alignment_pixels * alignment_pixels;
        if chunk_pixels == 0 {
            return Err("device cannot bind an aligned RGBA32F chunk");
        }
        if source == output {
            return Err("exposure requires separate buffers");
        }
        if !source.usage().contains(wgpu::BufferUsages::STORAGE)
            || !output.usage().contains(wgpu::BufferUsages::STORAGE)
        {
            return Err("buffers require STORAGE usage");
        }
        let mut offset_pixels = 0_u64;
        while offset_pixels < u64::from(count) {
            let current = (u64::from(count) - offset_pixels).min(chunk_pixels) as u32;
            let words = [stops.exp2().to_bits(), current, 0, 0];
            let parameters = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("linear exposure parameters"),
                contents: bytemuck::cast_slice(&words),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("linear exposure bindings"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: source,
                            offset: offset_pixels * 16,
                            size: std::num::NonZeroU64::new(u64::from(current) * 16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: output,
                            offset: offset_pixels * 16,
                            size: std::num::NonZeroU64::new(u64::from(current) * 16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: parameters.as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(current.div_ceil(64), 1, 1);
            drop(pass);
            offset_pixels += u64::from(current);
        }
        Ok(())
    }
}
