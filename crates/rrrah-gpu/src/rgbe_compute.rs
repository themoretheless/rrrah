//! Blocking RGBE interpolation with explicit CPU/GPU buffer admission.
use wgpu::util::DeviceExt;
#[derive(Debug, thiserror::Error)]
pub enum RgbeComputeError {
    #[error("RGBE compute cancelled")]
    Cancelled,
    #[error(transparent)]
    Plan(#[from] crate::RgbePlanError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("invalid RGBE input: {0}")]
    Invalid(&'static str),
    #[error("RGBE readback failed: {0}")]
    Readback(String),
}
#[derive(Debug)]
pub struct RgbeCompute {
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
}
impl RgbeCompute {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RGBE interpolation"),
            source: wgpu::ShaderSource::Wgsl(crate::RGBE_INTERPOLATION_SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RGBE interpolation"),
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
    /// Input accounting belongs to the caller. Output remains CPU-budgeted until
    /// its last owner drops. GPU accounting covers planned buffers, not drivers.
    /// Input must have explicitly resolved per-site black/white and four WB gains.
    pub fn execute_managed(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        quad: [u32; 4],
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, RgbeComputeError> {
        self.execute_managed_with_cancel(queue, source_values, dimensions, quad,
            gpu_budget, cpu_budget, || false)
    }
    /// Cooperative cancellation before submission and after completed readback.
    /// Submitted GPU work and driver waits are not interrupted.
    pub fn execute_managed_with_cancel(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        quad: [u32; 4],
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, RgbeComputeError> {
        if cancelled() { return Err(RgbeComputeError::Cancelled); }
        let plan = crate::RgbePlan::new(dimensions, quad, &self.device.limits())?;
        if source_values.len() as u64 != plan.source_bytes / 4 {
            return Err(RgbeComputeError::Invalid("sample count differs from geometry"));
        }
        for (index, value) in source_values.iter().enumerate() {
            if index % 4096 == 0 && cancelled() { return Err(RgbeComputeError::Cancelled); }
            if !value.is_finite() { return Err(RgbeComputeError::Invalid("samples must be finite")); }
        }
        if cancelled() { return Err(RgbeComputeError::Cancelled); }
        let cpu_reservation = cpu_budget.try_reserve(plan.output_bytes)?;
        let reservation = std::sync::Arc::new(gpu_budget.try_reserve(plan.gpu_buffer_bytes)?);
        if cancelled() { return Err(RgbeComputeError::Cancelled); }
        let mut result = cpu_reservation.try_buffer(source_values.len(), [0f32; 4])?;
        let source = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("managed RGBE input"),
            contents: bytemuck::cast_slice(source_values),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed RGBE output"),
            size: plan.output_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed RGBE readback"),
            size: plan.output_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for dispatch in plan.dispatches {
            if cancelled() { return Err(RgbeComputeError::Cancelled); }
            let params = [
                dimensions[0],
                dimensions[1],
                dispatch.first_row,
                dispatch.rows,
                quad[0],
                quad[1],
                quad[2],
                quad[3],
            ];
            let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("managed RGBE row uniform"),
                contents: bytemuck::cast_slice(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: source.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &output,
                            offset: dispatch.output_offset,
                            size: std::num::NonZeroU64::new(dispatch.output_bytes),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(dispatch.workgroups[0], dispatch.workgroups[1], 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, plan.output_bytes);
        if cancelled() { return Err(RgbeComputeError::Cancelled); }
        let submission = queue.submit([encoder.finish()]);
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
            .map_err(|e| RgbeComputeError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| RgbeComputeError::Readback(e.to_string()))?
            .map_err(|e| RgbeComputeError::Readback(e.to_string()))?;
        let mapped = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|e| RgbeComputeError::Readback(e.to_string()))?;
        result.copy_from_slice(bytemuck::cast_slice(&mapped));
        drop(mapped);
        readback.unmap();
        if cancelled() { return Err(RgbeComputeError::Cancelled); }
        Ok(result.freeze().into())
    }
}
