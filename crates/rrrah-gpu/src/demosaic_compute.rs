//! Managed GPU compute demosaicing for 2×2 Bayer sensors (Bilinear, SmoothHue, Malvar–He–Cutler 5×5,
//! Directional, VNG, RCD) with explicit CPU/GPU `MemoryBudget` admission, cooperative
//! cancellation, and resident / readback modes.

/// Selectable GPU shader and compute demosaicing algorithms for 2×2 Bayer sensors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GpuDemosaicAlgorithm {
    /// Fast 3×3 bilinear interpolation (minimum tile halo: 1).
    #[default]
    Bilinear,
    /// Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter (minimum tile halo: 2).
    MalvarHeCutler,
    /// Hamilton–Adams edge-directed colour-difference interpolation (nested filter halo: 3).
    Directional,
    /// Cok (1987) constant-hue-ratio (`R/G`, `B/G`) interpolation (minimum tile halo: 2).
    SmoothHue,
    /// Chang–Chen–chi (1999) Variable Number of Gradients (nested filter halo: 3).
    Vng,
    /// Luis Sanz Rodríguez Ratio Corrected Demosaicing (nested filter halo: 4).
    Rcd,
}

impl GpuDemosaicAlgorithm {
    /// Complete list of all supported GPU demosaicing algorithms.
    pub const ALL: [Self; 6] = [
        Self::Bilinear,
        Self::SmoothHue,
        Self::MalvarHeCutler,
        Self::Directional,
        Self::Vng,
        Self::Rcd,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bilinear => "bilinear",
            Self::MalvarHeCutler => "malvar-he-cutler",
            Self::Directional => "directional",
            Self::SmoothHue => "smooth-hue",
            Self::Vng => "vng",
            Self::Rcd => "rcd",
        }
    }

    /// Shader uniform code (`0` = Bilinear, `1` = MalvarHeCutler, `2` = Directional, `3` = SmoothHue, `4` = Vng, `5` = Rcd).
    #[must_use]
    pub const fn shader_code(self) -> u32 {
        match self {
            Self::Bilinear => 0,
            Self::MalvarHeCutler => 1,
            Self::Directional => 2,
            Self::SmoothHue => 3,
            Self::Vng => 4,
            Self::Rcd => 5,
        }
    }

    /// Minimum tile halo in sensor samples required so tile boundaries never clip the filter stencil.
    #[must_use]
    pub const fn min_tile_halo(self) -> u32 {
        match self {
            Self::Bilinear => 1,
            Self::MalvarHeCutler | Self::SmoothHue => 2,
            Self::Directional | Self::Vng => 3,
            Self::Rcd => 4,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BayerDemosaicError {
    #[error("Bayer GPU compute demosaic cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("invalid Bayer compute demosaic input: {0}")]
    Invalid(&'static str),
    #[error("Bayer compute demosaic readback failed: {0}")]
    Readback(String),
}

/// Queue-ordered linear RGBA32F demosaiced result retained on the GPU.
#[derive(Debug, Clone)]
pub struct ResidentBayerDemosaic {
    buffer: wgpu::Buffer,
    dimensions: [u32; 2],
    _reservation: std::sync::Arc<rrrah_core::Reservation>,
}

impl ResidentBayerDemosaic {
    #[must_use]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    #[must_use]
    pub fn dimensions(&self) -> [u32; 2] {
        self.dimensions
    }

    #[must_use]
    pub fn resource_lease(&self) -> crate::GpuResourceLease {
        crate::GpuResourceLease::new(std::iter::once(self._reservation.clone()))
    }
}

/// Calibrated scene-linear sRGB result; WB and camera matrix have already been applied.
#[derive(Debug, Clone)]
pub struct ResidentLinearBayer {
    pub(crate) inner: ResidentBayerDemosaic,
    pub(crate) crop: rrrah_core::Rect,
    pub(crate) orientation: rrrah_core::Orientation,
}
impl ResidentLinearBayer {
    pub fn buffer(&self) -> &wgpu::Buffer {
        self.inner.buffer()
    }
    pub fn dimensions(&self) -> [u32; 2] {
        self.inner.dimensions()
    }
    pub fn resource_lease(&self) -> crate::GpuResourceLease {
        self.inner.resource_lease()
    }
}

#[derive(Debug)]
pub struct BayerDemosaicCompute {
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
}

impl BayerDemosaicCompute {
    /// WGSL uses u32 linear indices and i32 reflected coordinates/periods.
    fn checked_sensor_count([width, height]: [u32; 2]) -> Result<u64, BayerDemosaicError> {
        if width < 2 || height < 2 {
            return Err(BayerDemosaicError::Invalid("dimensions must be at least 2x2"));
        }
        // reflect_coord computes 2 * (n - 1) in signed arithmetic.
        const MAX_REFLECTED_AXIS: u32 = (i32::MAX as u32) / 2 + 1;
        let count = u64::from(width) * u64::from(height);
        if width > MAX_REFLECTED_AXIS || height > MAX_REFLECTED_AXIS || count > u64::from(u32::MAX) {
            return Err(BayerDemosaicError::Invalid("geometry exceeds shader index range"));
        }
        Ok(count)
    }
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bayer 2x2 compute demosaic"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/bayer_demosaic.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bayer 2x2 compute demosaic"),
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

    fn validate_bayer_quad(cfa: [u32; 4]) -> Result<(), BayerDemosaicError> {
        let r = cfa.iter().filter(|&&c| c == 0).count();
        let g = cfa.iter().filter(|&&c| c == 1).count();
        let b = cfa.iter().filter(|&&c| c == 2).count();
        if (r, g, b) != (1, 2, 1) {
            return Err(BayerDemosaicError::Invalid(
                "CFA quad must be a valid 2x2 RGB Bayer layout",
            ));
        }
        Ok(())
    }

    /// Submit Bayer compute demosaicing and retain the RGBA32F output buffer on the GPU.
    pub fn execute_resident<F: FnMut() -> bool>(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        cfa: [u32; 4],
        algorithm: GpuDemosaicAlgorithm,
        gpu_budget: &rrrah_core::MemoryBudget,
        cancelled: F,
    ) -> Result<ResidentBayerDemosaic, BayerDemosaicError> {
        self.execute_resident_with_color(
            queue,
            source_values,
            dimensions,
            cfa,
            algorithm,
            gpu_budget,
            None,
            cancelled,
        )
    }

    /// Normalize native sensor levels/WB, then apply the camera matrix in the compute shader.
    pub fn execute_mosaic_resident(
        &self,
        queue: &wgpu::Queue,
        mosaic: &rrrah_core::DecodedMosaic,
        algorithm: GpuDemosaicAlgorithm,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<ResidentLinearBayer, BayerDemosaicError> {
        Self::checked_sensor_count([mosaic.metadata.width, mosaic.metadata.height])?;
        let (calibration, source) =
            rrrah_core::bayer::Calibration::normalize_managed(mosaic, cpu_budget, &mut cancelled).map_err(
                |error| match error {
                    rrrah_core::bayer::CalibrationError::Cancelled => BayerDemosaicError::Cancelled,
                    rrrah_core::bayer::CalibrationError::Memory(error) => BayerDemosaicError::Memory(error),
                    _ => BayerDemosaicError::Invalid("invalid native Bayer calibration"),
                },
            )?;
        let inner = self.execute_resident_with_color(
            queue,
            &source,
            [mosaic.metadata.width, mosaic.metadata.height],
            calibration.quad(),
            algorithm,
            gpu_budget,
            Some(calibration.camera_to_rgb()),
            cancelled,
        )?;
        Ok(ResidentLinearBayer {
            inner,
            crop: mosaic.metadata.effective_crop(),
            orientation: mosaic.metadata.orientation,
        })
    }

    /// Submit Bayer compute demosaicing and retain the RGBA32F output buffer on the GPU.
    fn execute_resident_with_color<F: FnMut() -> bool>(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        cfa: [u32; 4],
        algorithm: GpuDemosaicAlgorithm,
        gpu_budget: &rrrah_core::MemoryBudget,
        color: Option<[[f32; 3]; 3]>,
        mut cancelled: F,
    ) -> Result<ResidentBayerDemosaic, BayerDemosaicError> {
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        Self::validate_bayer_quad(cfa)?;
        let matrix_norm = color.map(|matrix| {
            matrix
                .iter()
                .map(|row| row.iter().map(|v| f64::from(*v).abs()).sum::<f64>())
                .fold(1.0f64, f64::max)
        });
        if color.is_some_and(|matrix| matrix.iter().flatten().any(|v| !v.is_finite())) {
            return Err(BayerDemosaicError::Invalid("nonfinite camera matrix"));
        }
        let [w, h] = dimensions;
        let count = Self::checked_sensor_count(dimensions)?;
        if source_values.len() as u64 != count {
            return Err(BayerDemosaicError::Invalid("sample count differs from geometry"));
        }
        for (i, &v) in source_values.iter().enumerate() {
            if i % 4096 == 0 && cancelled() {
                return Err(BayerDemosaicError::Cancelled);
            }
            if matrix_norm.is_some_and(|norm| f64::from(v).abs() > f64::from(f32::MAX).sqrt() / (16.0 * norm))
            {
                return Err(BayerDemosaicError::Invalid("unsafe calibrated filter range"));
            }
            if !v.is_finite() {
                return Err(BayerDemosaicError::Invalid("samples must be finite"));
            }
        }
        let output_bytes = count * 16;
        let limits = self.device.limits();
        let binding_limit = u64::from(limits.max_storage_buffer_binding_size).min(limits.max_buffer_size);
        if w.div_ceil(8) > limits.max_compute_workgroups_per_dimension {
            return Err(BayerDemosaicError::Invalid("width exceeds device limit"));
        }
        let halo = algorithm.min_tile_halo();
        let source_row_limit = binding_limit / (u64::from(w) * 4);
        let source_interior_rows = source_row_limit.saturating_sub(u64::from(halo) * 2);
        let rows = (binding_limit / (u64::from(w) * 16))
            .min(source_interior_rows)
            .min(u64::from(limits.max_compute_workgroups_per_dimension) * 8)
            .min(u64::from(h)) as u32;
        if rows == 0 {
            return Err(BayerDemosaicError::Invalid("output row exceeds device limit"));
        }
        let window_bytes = u64::from(rows) * u64::from(w) * 16;
        let source_bytes = u64::from(h.min(rows.saturating_add(halo * 2))) * u64::from(w) * 4;
        if output_bytes > limits.max_buffer_size {
            return Err(BayerDemosaicError::Invalid(
                "resident output exceeds buffer limit",
            ));
        }
        let resident = std::sync::Arc::new(gpu_budget.try_reserve(output_bytes)?);
        let gpu_reservation = std::sync::Arc::new(gpu_budget.try_reserve(source_bytes + window_bytes + 96)?);
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        let source = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic input window"),
            size: source_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic output"),
            size: window_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let resident_output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resident Bayer demosaic output"),
            size: output_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic uniform"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for first_row in (0..h).step_by(rows as usize) {
            let active_rows = rows.min(h - first_row);
            let first_pixel = first_row * w;
            let active_pixels = active_rows * w;
            let active_bytes = u64::from(active_pixels) * 16;
            let source_first = first_row.saturating_sub(halo) * w;
            let source_end = (first_row + active_rows).saturating_add(halo).min(h) * w;
            let mut pending_writes = crate::QueuedUploadReservation {
                queue: queue.clone(),
                reservation: None,
                resources: crate::GpuResourceLease::new([gpu_reservation.clone(), resident.clone()]),
            };
            queue.write_buffer(
                &source,
                0,
                bytemuck::cast_slice(&source_values[source_first as usize..source_end as usize]),
            );
            let mut params: [u32; 24] = [
                w,
                h,
                first_row,
                active_rows,
                cfa[0],
                cfa[1],
                cfa[2],
                cfa[3],
                first_pixel,
                source_first,
                algorithm.shader_code(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            if let Some(matrix) = color {
                params[11] = 1;
                for row in 0..3 {
                    for column in 0..3 {
                        params[12 + row * 4 + column] = matrix[row][column].to_bits();
                    }
                }
            }
            queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&params));
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("managed Bayer demosaic bind group"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: source.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(w.div_ceil(8), active_rows.div_ceil(8), 1);
            }
            encoder.copy_buffer_to_buffer(
                &output,
                0,
                &resident_output,
                u64::from(first_pixel) * 16,
                active_bytes,
            );
            if cancelled() {
                return Err(BayerDemosaicError::Cancelled);
            }
            queue.submit([encoder.finish()]);
            pending_writes.resources = crate::GpuResourceLease::default();
            let in_flight = gpu_reservation.clone();
            let pending_output = resident.clone();
            queue.on_submitted_work_done(move || drop((in_flight, pending_output)));
        }
        Ok(ResidentBayerDemosaic {
            buffer: resident_output,
            dimensions,
            _reservation: resident,
        })
    }

    /// Blocking, CPU/GPU budgeted Bayer compute demosaicing with readback to `PixelBuffer<[f32; 4]>`.
    pub fn execute_managed(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        cfa: [u32; 4],
        algorithm: GpuDemosaicAlgorithm,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, BayerDemosaicError> {
        self.execute_managed_with_cancel(
            queue,
            source_values,
            dimensions,
            cfa,
            algorithm,
            gpu_budget,
            cpu_budget,
            || false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute_managed_with_cancel(
        &self,
        queue: &wgpu::Queue,
        source_values: &[f32],
        dimensions: [u32; 2],
        cfa: [u32; 4],
        algorithm: GpuDemosaicAlgorithm,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, BayerDemosaicError> {
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        Self::validate_bayer_quad(cfa)?;
        let [w, h] = dimensions;
        let count = Self::checked_sensor_count(dimensions)?;
        if source_values.len() as u64 != count {
            return Err(BayerDemosaicError::Invalid("sample count differs from geometry"));
        }
        for (i, &v) in source_values.iter().enumerate() {
            if i % 4096 == 0 && cancelled() {
                return Err(BayerDemosaicError::Cancelled);
            }
            if !v.is_finite() {
                return Err(BayerDemosaicError::Invalid("samples must be finite"));
            }
        }
        let output_bytes = count * 16;
        let limits = self.device.limits();
        let binding_limit = u64::from(limits.max_storage_buffer_binding_size).min(limits.max_buffer_size);
        if w.div_ceil(8) > limits.max_compute_workgroups_per_dimension {
            return Err(BayerDemosaicError::Invalid("width exceeds device limit"));
        }
        let halo = algorithm.min_tile_halo();
        let source_row_limit = binding_limit / (u64::from(w) * 4);
        let source_interior_rows = source_row_limit.saturating_sub(u64::from(halo) * 2);
        let rows = (binding_limit / (u64::from(w) * 16))
            .min(source_interior_rows)
            .min(u64::from(limits.max_compute_workgroups_per_dimension) * 8)
            .min(u64::from(h)) as u32;
        if rows == 0 {
            return Err(BayerDemosaicError::Invalid("output row exceeds device limit"));
        }
        let window_bytes = u64::from(rows) * u64::from(w) * 16;
        let source_bytes = u64::from(h.min(rows.saturating_add(halo * 2))) * u64::from(w) * 4;
        let cpu_reservation = cpu_budget.try_reserve(output_bytes)?;
        let gpu_reservation =
            std::sync::Arc::new(gpu_budget.try_reserve(source_bytes + window_bytes * 2 + 96)?);
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        let mut result = cpu_reservation.try_buffer(source_values.len(), [0.0f32; 4])?;
        let source = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic input window"),
            size: source_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic output"),
            size: window_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic readback"),
            size: window_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic uniform"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for first_row in (0..h).step_by(rows as usize) {
            let active_rows = rows.min(h - first_row);
            let first_pixel = first_row * w;
            let active_pixels = active_rows * w;
            let active_bytes = u64::from(active_pixels) * 16;
            let source_first = first_row.saturating_sub(halo) * w;
            let source_end = (first_row + active_rows).saturating_add(halo).min(h) * w;
            let mut pending_writes = crate::QueuedUploadReservation {
                queue: queue.clone(),
                reservation: None,
                resources: crate::GpuResourceLease::new([gpu_reservation.clone()]),
            };
            queue.write_buffer(
                &source,
                0,
                bytemuck::cast_slice(&source_values[source_first as usize..source_end as usize]),
            );
            let params: [u32; 24] = [
                w,
                h,
                first_row,
                active_rows,
                cfa[0],
                cfa[1],
                cfa[2],
                cfa[3],
                first_pixel,
                source_first,
                algorithm.shader_code(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&params));
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("managed Bayer demosaic bind group"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: source.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(w.div_ceil(8), active_rows.div_ceil(8), 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, active_bytes);
            if cancelled() {
                return Err(BayerDemosaicError::Cancelled);
            }
            let submission = queue.submit([encoder.finish()]);
            pending_writes.resources = crate::GpuResourceLease::default();
            let in_flight = gpu_reservation.clone();
            queue.on_submitted_work_done(move || drop(in_flight));
            let (tx, rx) = std::sync::mpsc::channel();
            readback
                .slice(..active_bytes)
                .map_async(wgpu::MapMode::Read, move |status| {
                    let _ = tx.send(status);
                });
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
            rx.recv()
                .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?
                .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
            let mapped = readback
                .slice(..active_bytes)
                .get_mapped_range()
                .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
            let copied = (|| {
                for (dst, src) in result[first_pixel as usize..(first_pixel + active_pixels) as usize]
                    .chunks_mut(4096)
                    .zip(bytemuck::cast_slice::<u8, [f32; 4]>(&mapped).chunks(4096))
                {
                    if cancelled() {
                        return Err(BayerDemosaicError::Cancelled);
                    }
                    dst.copy_from_slice(src);
                }
                Ok(())
            })();
            drop(mapped);
            readback.unmap();
            copied?;
        }
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        Ok(result.freeze().into())
    }
}

#[cfg(test)]
mod tests {
    use super::{BayerDemosaicCompute, BayerDemosaicError, GpuDemosaicAlgorithm};

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        test_device_with_binding_limit(None, None)
    }

    fn test_device_with_binding_limit(
        binding_limit: Option<u32>,
        max_buffer: Option<u64>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        let required = std::env::var("RRRAH_GPU_OPTIONAL").as_deref() == Ok("0");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if std::env::var("RRRAH_GPU_BACKEND").as_deref() == Ok("metal") {
            descriptor.backends = wgpu::Backends::METAL;
        }
        let instance = wgpu::Instance::new(descriptor);
        let result = (|| {
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
            eprintln!("Bayer compute adapter: {:?}", adapter.get_info());
            let mut descriptor = wgpu::DeviceDescriptor::default();
            if let Some(limit) = binding_limit {
                descriptor.required_limits.max_storage_buffer_binding_size = u64::from(limit);
            }
            if let Some(bytes) = max_buffer {
                descriptor.required_limits.max_buffer_size = bytes;
            }
            pollster::block_on(adapter.request_device(&descriptor)).ok()
        })();
        assert!(
            !required || result.is_some(),
            "required Bayer compute GPU unavailable"
        );
        result
    }

    #[test]
    fn bayer_demosaic_compute_shader_validates_with_naga() {
        let module = naga::front::wgsl::parse_str(include_str!("../shaders/bayer_demosaic.wgsl"))
            .expect("bayer_demosaic.wgsl must parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .expect("bayer_demosaic.wgsl must pass Naga validation");
    }

    #[test]
    fn bayer_demosaic_compute_algorithms_preserve_constant_hdr_and_sharpen_edges_on_gpu() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let compute = BayerDemosaicCompute::new(&device);
        let gpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let cpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);

        let w = 24u32;
        let h = 24u32;
        let cfa = [0u32, 1, 1, 2]; // RGGB
        let mut const_mosaic = vec![0.0f32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let phase = ((y & 1) * 2 + (x & 1)) as usize;
                const_mosaic[(y * w + x) as usize] = match cfa[phase] {
                    0 => 2.5,
                    1 => 1.75,
                    _ => 0.5,
                };
            }
        }

        for alg in GpuDemosaicAlgorithm::ALL {
            let out = compute
                .execute_managed(&queue, &const_mosaic, [w, h], cfa, alg, &gpu_budget, &cpu_budget)
                .expect("constant HDR demosaic must succeed");
            for y in 3..(h - 3) {
                for x in 3..(w - 3) {
                    let px = out[(y * w + x) as usize];
                    assert!((px[0] - 2.5).abs() < 1e-4, "{alg:?} R mismatch: {}", px[0]);
                    assert!((px[1] - 1.75).abs() < 1e-4, "{alg:?} G mismatch: {}", px[1]);
                    assert!((px[2] - 0.5).abs() < 1e-4, "{alg:?} B mismatch: {}", px[2]);
                    assert!((px[3] - 1.0).abs() < 1e-6);
                }
            }
            drop(out);
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            assert_eq!(cpu_budget.used(), 0);
            assert_eq!(gpu_budget.used(), 0);
        }

        // Sharp vertical stripe pattern: Directional, RCD, and MalvarHeCutler must achieve lower MSE than Bilinear
        let mut stripe_mosaic = vec![0.0f32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let lum = if (x / 2) % 2 == 0 { 0.85 } else { 0.15 };
                stripe_mosaic[(y * w + x) as usize] = lum;
            }
        }
        let mse = |alg: GpuDemosaicAlgorithm| -> f64 {
            let out = compute
                .execute_managed(&queue, &stripe_mosaic, [w, h], cfa, alg, &gpu_budget, &cpu_budget)
                .unwrap();
            let mut err = 0.0f64;
            let mut n = 0usize;
            for y in 4..(h - 4) {
                for x in 4..(w - 4) {
                    let gt = if (x / 2) % 2 == 0 { 0.85f64 } else { 0.15f64 };
                    let px = out[(y * w + x) as usize];
                    for c in 0..3 {
                        let d = f64::from(px[c]) - gt;
                        err += d * d;
                        n += 1;
                    }
                }
            }
            err / (n as f64)
        };
        let mse_bil = mse(GpuDemosaicAlgorithm::Bilinear);
        let mse_mhc = mse(GpuDemosaicAlgorithm::MalvarHeCutler);
        let mse_dir = mse(GpuDemosaicAlgorithm::Directional);
        let mse_rcd = mse(GpuDemosaicAlgorithm::Rcd);
        assert!(
            mse_dir < mse_bil,
            "Directional ({mse_dir}) must beat Bilinear ({mse_bil})"
        );
        assert!(
            mse_rcd < mse_bil,
            "RCD ({mse_rcd}) must beat Bilinear ({mse_bil})"
        );
        assert!(
            mse_mhc <= mse_bil + 1e-5,
            "MHC ({mse_mhc}) must not degrade Bilinear ({mse_bil})"
        );
    }

    #[test]
    fn calibrated_native_bayer_resident_preserves_signed_linear_hdr() {
        use rrrah_core::{
            CfaColor::*, CfaPattern, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
        };
        let Some((device, queue)) = test_device() else {
            return;
        };
        let metadata = RawMetadata {
            make: "authored".into(),
            model: "calibrated Bayer compute".into(),
            width: 17,
            height: 13,
            components_per_pixel: 1,
            bits_per_sample: 16,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![Red, Green, Green, Blue],
            }),
            black_level: LevelGrid {
                width: 3,
                height: 5,
                components: 1,
                values: (0..15).map(|i| (i * 4) as f32).collect(),
            },
            white_level: WhiteLevel(vec![1000., 2000., 3000., 4000.]),
            white_balance: [2., 1., 0.5, 1.],
            xyz_to_camera: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        let values: Vec<u16> = (0..13)
            .flat_map(|y| {
                let metadata = &metadata;
                (0..17).map(move |x| {
                    let phase = ((y % 2) * 2 + x % 2) as usize;
                    let black = metadata.black_level.at(y, x, 0).unwrap();
                    let normalized_before_wb = [0.25, 2., 2., 1.][phase];
                    (black + (metadata.white_level.0[phase] - black) * normalized_before_wb) as u16
                })
            })
            .collect();
        let mosaic = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(values)).unwrap();
        let calibration = rrrah_core::bayer::Calibration::new(&mosaic.metadata).unwrap();
        let expected = rrrah_core::apply_3x3(calibration.camera_to_rgb(), [0.5, 2., 0.5]);
        assert!(expected[0] < 0. && expected[1] > 1.);
        let gpu = rrrah_core::MemoryBudget::new(1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(17 * 13 * 4);
        let compute = BayerDemosaicCompute::new(&device);
        for algorithm in GpuDemosaicAlgorithm::ALL {
            let output = compute
                .execute_mosaic_resident(&queue, &mosaic, algorithm, &gpu, &cpu, || false)
                .unwrap();
            assert_eq!(cpu.used(), 0);
            let bytes = 17 * 13 * 16;
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("calibrated Bayer qualification"),
                size: bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(output.buffer(), 0, &readback, 0, bytes);
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
                tx.send(status).unwrap();
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = readback.slice(..).get_mapped_range().unwrap();
            for pixel in bytemuck::cast_slice::<u8, [f32; 4]>(&mapped) {
                for channel in 0..3 {
                    assert!(
                        (pixel[channel] - expected[channel]).abs() < 5e-5,
                        "{algorithm:?}: {pixel:?} expected={expected:?}"
                    );
                }
                assert_eq!(pixel[3], 1.);
            }
            drop(mapped);
            readback.unmap();
            assert_eq!(gpu.used(), bytes);
            drop(output);
            assert_eq!(gpu.used(), 0);
        }
        let error = compute
            .execute_mosaic_resident(
                &queue,
                &mosaic,
                GpuDemosaicAlgorithm::Bilinear,
                &gpu,
                &cpu,
                || cpu.used() > 0,
            )
            .unwrap_err();
        assert!(matches!(error, BayerDemosaicError::Cancelled));
        assert_eq!((cpu.used(), gpu.used()), (0, 0));
        eprintln!(
            "native Bayer calibrated resident: six algorithms, odd repeat grid, phase white, signed RGB and HDR, alpha1, final credits zero"
        );
    }

    #[test]
    fn bayer_output_windows_match_whole_frame_all_algorithms_and_cfa_phases() {
        let Some((whole_device, whole_queue)) = test_device() else {
            return;
        };
        let Some((device, queue)) = test_device_with_binding_limit(Some(65536), None) else {
            return;
        };
        let whole = BayerDemosaicCompute::new(&whole_device);
        let windowed = BayerDemosaicCompute::new(&device);
        let gpu = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let dimensions = [129, 257];
        let source: Vec<f32> = (0..129 * 257)
            .map(|i| ((i % 129) * 13 + (i / 129) * 17) as f32 / 128.0 + (i % 19) as f32 / 32.0)
            .collect();
        assert!(source.len() * 4 > 65536);
        assert!(source.len() * 16 > 2 * 65536);
        for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
            for algorithm in GpuDemosaicAlgorithm::ALL {
                let reference = whole
                    .execute_managed(&whole_queue, &source, dimensions, cfa, algorithm, &gpu, &cpu)
                    .unwrap()
                    .to_vec();
                let resident = windowed
                    .execute_resident(&queue, &source, dimensions, cfa, algorithm, &gpu, || false)
                    .unwrap();
                let bytes = (source.len() * 16) as u64;
                let readback = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("resident Bayer qualification readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(resident.buffer(), 0, &readback, 0, bytes);
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                readback.slice(..).map_async(wgpu::MapMode::Read, move |status| {
                    tx.send(status).unwrap();
                });
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: None,
                    })
                    .unwrap();
                rx.recv().unwrap().unwrap();
                assert_eq!(gpu.used(), bytes);
                let mapped = readback.slice(..).get_mapped_range().unwrap();
                let resident_pixels = bytemuck::cast_slice::<u8, [f32; 4]>(&mapped);
                for (index, (actual, expected)) in resident_pixels.iter().zip(&reference).enumerate() {
                    assert_eq!(
                        actual, expected,
                        "resident {algorithm:?} CFA={cfa:?} pixel={index}"
                    );
                }
                drop(mapped);
                readback.unmap();
                drop(resident);
                assert_eq!(gpu.used(), 0);
                for cutoff in [2, 3] {
                    let mut checks = 0;
                    let error = windowed
                        .execute_resident(&queue, &source, dimensions, cfa, algorithm, &gpu, || {
                            if gpu.used() > 0 {
                                checks += 1;
                            }
                            checks == cutoff
                        })
                        .unwrap_err();
                    assert!(matches!(error, BayerDemosaicError::Cancelled));
                    assert_eq!(checks, cutoff);
                    assert!(
                        gpu.used() > 0,
                        "queued source writes must retain GPU credits until polling"
                    );
                    device
                        .poll(wgpu::PollType::Wait {
                            submission_index: None,
                            timeout: None,
                        })
                        .unwrap();
                    assert_eq!(gpu.used(), 0);
                }
                let actual = windowed
                    .execute_managed(&queue, &source, dimensions, cfa, algorithm, &gpu, &cpu)
                    .unwrap();
                for (index, (actual, expected)) in actual.iter().zip(&reference).enumerate() {
                    assert_eq!(actual, expected, "{algorithm:?} CFA={cfa:?} pixel={index}");
                }
                drop(actual);
                let mut admitted_checks = 0;
                let error = windowed
                    .execute_managed_with_cancel(
                        &queue,
                        &source,
                        dimensions,
                        cfa,
                        algorithm,
                        &gpu,
                        &cpu,
                        || {
                            if gpu.used() > 0 {
                                admitted_checks += 1;
                            }
                            admitted_checks == 4
                        },
                    )
                    .unwrap_err();
                assert!(matches!(error, BayerDemosaicError::Cancelled));
                assert_eq!(admitted_checks, 4);
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: None,
                    })
                    .unwrap();
                assert_eq!((gpu.used(), cpu.used()), (0, 0));
                let retry = windowed
                    .execute_managed(&queue, &source, dimensions, cfa, algorithm, &gpu, &cpu)
                    .unwrap();
                assert_eq!(&*retry, reference.as_slice());
                drop(retry);
                assert_eq!((gpu.used(), cpu.used()), (0, 0));
                eprintln!(
                    "Bayer windows {algorithm:?} CFA={cfa:?}: nine source/output strips, every pixel exact, second-strip cancellation/retry exact, credits zero"
                );
            }
        }
    }

    #[test]
    #[ignore = "full-sensor Bayer output and CPU reference exceed 500MB"]
    fn full_sensor_bayer_windows_match_independent_bilinear_every_pixel() {
        let Some((device, queue)) = test_device_with_binding_limit(None, Some(407912960)) else {
            return;
        };
        let [w, h] = [6188u32, 4120u32];
        let sample = |x: u32, y: u32| ((x * 13 + y * 17) % 257) as f32 / 32.0;
        let source: Vec<f32> = (0..h).flat_map(|y| (0..w).map(move |x| sample(x, y))).collect();
        let gpu = rrrah_core::MemoryBudget::new(768 * 1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(u64::from(w) * u64::from(h) * 16);
        let started = std::time::Instant::now();
        let output = BayerDemosaicCompute::new(&device)
            .execute_managed(
                &queue,
                &source,
                [w, h],
                [0, 1, 1, 2],
                GpuDemosaicAlgorithm::Bilinear,
                &gpu,
                &cpu,
            )
            .unwrap();
        let reflect = |i: i32, n: u32| {
            let period = 2 * (n as i32 - 1);
            let v = i.rem_euclid(period);
            if v >= n as i32 {
                (period - v) as u32
            } else {
                v as u32
            }
        };
        let expected_pixel = |x: u32, y: u32| {
            let at = |dx: i32, dy: i32| sample(reflect(x as i32 + dx, w), reflect(y as i32 + dy, h));
            let horizontal = (at(-1, 0) + at(1, 0)) * 0.5;
            let vertical = (at(0, -1) + at(0, 1)) * 0.5;
            let axial = (at(0, -1) + at(0, 1) + at(-1, 0) + at(1, 0)) * 0.25;
            let diagonal = (at(-1, -1) + at(1, -1) + at(-1, 1) + at(1, 1)) * 0.25;
            let own = at(0, 0);
            match (y % 2, x % 2) {
                (0, 0) => [own, axial, diagonal, 1.0],
                (1, 1) => [diagonal, axial, own, 1.0],
                (0, 1) => [horizontal, own, vertical, 1.0],
                _ => [vertical, own, horizontal, 1.0],
            }
        };
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    output[(y * w + x) as usize],
                    expected_pixel(x, y),
                    "sensor {x},{y}"
                );
            }
        }
        assert_eq!(gpu.used(), 0);
        drop(output);
        assert_eq!(cpu.used(), 0);
        let compute = BayerDemosaicCompute::new(&device);
        let resident = compute
            .execute_resident(
                &queue,
                &source,
                [w, h],
                [0, 1, 1, 2],
                GpuDemosaicAlgorithm::Bilinear,
                &gpu,
                || false,
            )
            .unwrap();
        let chunk_pixels = 1024 * 1024u64;
        let total_pixels = u64::from(w) * u64::from(h);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("full sensor resident Bayer qualification readback"),
            size: chunk_pixels * 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for first in (0..total_pixels).step_by(chunk_pixels as usize) {
            let pixels = chunk_pixels.min(total_pixels - first);
            let bytes = pixels * 16;
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(resident.buffer(), first * 16, &readback, 0, bytes);
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback
                .slice(..bytes)
                .map_async(wgpu::MapMode::Read, move |status| {
                    tx.send(status).unwrap();
                });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = readback.slice(..bytes).get_mapped_range().unwrap();
            for (offset, actual) in bytemuck::cast_slice::<u8, [f32; 4]>(&mapped).iter().enumerate() {
                let index = first + offset as u64;
                assert_eq!(
                    *actual,
                    expected_pixel((index % u64::from(w)) as u32, (index / u64::from(w)) as u32),
                    "resident pixel {index}"
                );
            }
            drop(mapped);
            readback.unmap();
        }
        assert_eq!(gpu.used(), total_pixels * 16);
        let shared = resident.clone();
        drop(resident);
        assert_eq!(gpu.used(), total_pixels * 16);
        drop(shared);
        assert_eq!(gpu.used(), 0);
        eprintln!(
            "Bayer resident full sensor: all25494560 pixels independently exact, retained clone credit exact, final0, readback chunk16MiB"
        );
        eprintln!(
            "Bayer full sensor {w}x{h}: all pixels independently exact, HDR, reflected edges, output windows, GPU peak={}, CPU final=0 GPU final=0 total_ms={}",
            gpu.peak(),
            started.elapsed().as_millis()
        );
    }

    #[test]
    fn bayer_compute_admitted_cancellation_retries_every_algorithm_exactly() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let compute = BayerDemosaicCompute::new(&device);
        let gpu = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let cpu = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let gpu_owner = gpu.try_reserve(1024).unwrap();
        let cpu_owner = cpu.try_reserve(4096).unwrap();
        let dimensions = [129, 65];
        let source: Vec<f32> = (0..129 * 65).map(|i| (i % 137) as f32 / 32.0).collect();
        for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
            for algorithm in GpuDemosaicAlgorithm::ALL {
                let baseline = compute
                    .execute_managed(&queue, &source, dimensions, cfa, algorithm, &gpu, &cpu)
                    .unwrap()
                    .to_vec();
                assert_eq!((gpu.used(), cpu.used()), (1024, 4096));
                // Admission, encoded work, three readback chunks, final return.
                for cutoff in 1..=6 {
                    let mut admitted_checks = 0;
                    let error = compute
                        .execute_managed_with_cancel(
                            &queue,
                            &source,
                            dimensions,
                            cfa,
                            algorithm,
                            &gpu,
                            &cpu,
                            || {
                                if gpu.used() > 1024 {
                                    assert!(cpu.used() > 4096);
                                    admitted_checks += 1;
                                }
                                admitted_checks >= cutoff
                            },
                        )
                        .unwrap_err();
                    assert!(matches!(error, BayerDemosaicError::Cancelled));
                    assert_eq!(admitted_checks, cutoff);
                    device
                        .poll(wgpu::PollType::Wait {
                            submission_index: None,
                            timeout: None,
                        })
                        .unwrap();
                    assert_eq!((gpu.used(), cpu.used()), (1024, 4096));
                    let retry = compute
                        .execute_managed(&queue, &source, dimensions, cfa, algorithm, &gpu, &cpu)
                        .unwrap();
                    assert_eq!(retry.len(), baseline.len());
                    assert!(
                        retry
                            .iter()
                            .zip(&baseline)
                            .all(|(a, b)| a.map(f32::to_bits) == b.map(f32::to_bits)),
                        "{algorithm:?} cfa={cfa:?} cutoff={cutoff}"
                    );
                    drop(retry);
                    assert_eq!((gpu.used(), cpu.used()), (1024, 4096));
                }
                eprintln!(
                    "Bayer {algorithm:?} cfa={cfa:?}: six admitted cancellations, exact retries, unrelated GPU1024/CPU4096 preserved"
                );
            }
        }
        drop(gpu_owner);
        drop(cpu_owner);
        assert_eq!((gpu.used(), cpu.used()), (0, 0));
        eprintln!(
            "Bayer cancellation matrix: algorithms=6 cfa=4 cutoffs=6 cancellations=144 exact_retries=144 CPU/GPU final=0"
        );
    }

    #[test]
    fn sensor_geometry_stays_within_wgsl_integer_ranges() {
        let max_axis = (i32::MAX as u32) / 2 + 1;
        assert_eq!(
            BayerDemosaicCompute::checked_sensor_count([6188, 4120]).unwrap(),
            25_494_560
        );
        assert_eq!(
            BayerDemosaicCompute::checked_sensor_count([65535, 65537]).unwrap(),
            u64::from(u32::MAX)
        );
        assert!(BayerDemosaicCompute::checked_sensor_count([65536, 65536]).is_err());
        for geometry in [
            [max_axis + 1, 2],
            [2, max_axis + 1],
            [u32::MAX, u32::MAX],
            [1, 2],
            [2, 0],
        ] {
            assert!(
                BayerDemosaicCompute::checked_sensor_count(geometry).is_err(),
                "{geometry:?}"
            );
        }
        assert_eq!(
            BayerDemosaicCompute::checked_sensor_count([max_axis, 2]).unwrap(),
            u64::from(max_axis) * 2
        );
    }

    #[test]
    fn bayer_demosaic_compute_rejects_invalid_inputs_and_cancels_cleanly() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let compute = BayerDemosaicCompute::new(&device);
        let gpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let cpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);

        // Reject shader-address overflow before sample admission or GPU allocation.
        for dimensions in [[65536, 65536], [(i32::MAX as u32) / 2 + 2, 2]] {
            assert!(matches!(
                compute.execute_managed(
                    &queue,
                    &[],
                    dimensions,
                    [0, 1, 1, 2],
                    GpuDemosaicAlgorithm::Bilinear,
                    &gpu_budget,
                    &cpu_budget
                ),
                Err(BayerDemosaicError::Invalid("geometry exceeds shader index range"))
            ));
            assert!(matches!(
                compute.execute_resident(
                    &queue,
                    &[],
                    dimensions,
                    [0, 1, 1, 2],
                    GpuDemosaicAlgorithm::Bilinear,
                    &gpu_budget,
                    || false
                ),
                Err(BayerDemosaicError::Invalid("geometry exceeds shader index range"))
            ));
            assert_eq!(cpu_budget.used(), 0);
            assert_eq!(gpu_budget.used(), 0);
        }

        // Invalid CFA quad (not 1 R, 2 G, 1 B)
        let err = compute
            .execute_managed(
                &queue,
                &[0.5; 16],
                [4, 4],
                [0, 0, 1, 2],
                GpuDemosaicAlgorithm::Bilinear,
                &gpu_budget,
                &cpu_budget,
            )
            .unwrap_err();
        assert!(matches!(err, BayerDemosaicError::Invalid(_)));

        // Cancelled execution releases all reservations
        let err = compute
            .execute_managed_with_cancel(
                &queue,
                &[0.5; 16],
                [4, 4],
                [0, 1, 1, 2],
                GpuDemosaicAlgorithm::Directional,
                &gpu_budget,
                &cpu_budget,
                || true,
            )
            .unwrap_err();
        assert!(matches!(err, BayerDemosaicError::Cancelled));
        assert_eq!(cpu_budget.used(), 0);
        assert_eq!(gpu_budget.used(), 0);
    }
}
