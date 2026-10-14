//! Managed GPU compute demosaicing for 2×2 Bayer sensors (Bilinear, SmoothHue, Malvar–He–Cutler 5×5,
//! Directional 5×5, VNG 5×5, RCD 7×7) with explicit CPU/GPU `MemoryBudget` admission, cooperative
//! cancellation, and resident / readback modes.

use wgpu::util::DeviceExt;

/// Selectable GPU shader and compute demosaicing algorithms for 2×2 Bayer sensors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GpuDemosaicAlgorithm {
    /// Fast 3×3 bilinear interpolation (minimum tile halo: 1).
    #[default]
    Bilinear,
    /// Malvar–He–Cutler (2004) 5×5 gradient-corrected linear filter (minimum tile halo: 2).
    MalvarHeCutler,
    /// Hamilton–Adams 5×5 edge-directed colour-difference interpolation (minimum tile halo: 2).
    Directional,
    /// Cok (1987) constant-hue-ratio (`R/G`, `B/G`) interpolation (minimum tile halo: 2).
    SmoothHue,
    /// Chang–Chen–chi (1999) 8-direction Variable Number of Gradients 5×5 (minimum tile halo: 2).
    Vng,
    /// Luis Sanz Rodríguez Ratio Corrected Demosaicing 7×7 (minimum tile halo: 3).
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
            Self::MalvarHeCutler | Self::Directional | Self::SmoothHue | Self::Vng => 2,
            Self::Rcd => 3,
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

#[derive(Debug)]
pub struct BayerDemosaicCompute {
    device: wgpu::Device,
    pipeline: wgpu::ComputePipeline,
}

impl BayerDemosaicCompute {
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
        mut cancelled: F,
    ) -> Result<ResidentBayerDemosaic, BayerDemosaicError> {
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        Self::validate_bayer_quad(cfa)?;
        let [w, h] = dimensions;
        if w < 2 || h < 2 {
            return Err(BayerDemosaicError::Invalid("dimensions must be at least 2x2"));
        }
        let count = u64::from(w)
            .checked_mul(u64::from(h))
            .ok_or(BayerDemosaicError::Invalid("dimension overflow"))?;
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
        let source_bytes = count * 4;
        let output_bytes = count * 16;
        let limits = self.device.limits();
        if output_bytes > limits.max_buffer_size
            || output_bytes > u64::from(limits.max_storage_buffer_binding_size)
        {
            return Err(BayerDemosaicError::Invalid("buffer exceeds device limit"));
        }
        let transient = std::sync::Arc::new(gpu_budget.try_reserve(source_bytes + 64)?);
        let resident = std::sync::Arc::new(gpu_budget.try_reserve(output_bytes)?);
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        let source = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resident Bayer demosaic input"),
            contents: bytemuck::cast_slice(source_values),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("resident Bayer demosaic output"),
            size: output_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params: [u32; 12] = [
            w,
            h,
            0,
            h,
            cfa[0],
            cfa[1],
            cfa[2],
            cfa[3],
            0,
            0,
            algorithm.shader_code(),
            0,
        ];
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resident Bayer demosaic uniform"),
            contents: bytemuck::cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident Bayer demosaic bind group"),
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
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        queue.submit([encoder.finish()]);
        let pending = resident.clone();
        queue.on_submitted_work_done(move || drop((transient, pending)));
        Ok(ResidentBayerDemosaic {
            buffer: output,
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
        if w < 2 || h < 2 {
            return Err(BayerDemosaicError::Invalid("dimensions must be at least 2x2"));
        }
        let count = u64::from(w)
            .checked_mul(u64::from(h))
            .ok_or(BayerDemosaicError::Invalid("dimension overflow"))?;
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
        let source_bytes = count * 4;
        let output_bytes = count * 16;
        let limits = self.device.limits();
        if output_bytes > limits.max_buffer_size
            || output_bytes > u64::from(limits.max_storage_buffer_binding_size)
        {
            return Err(BayerDemosaicError::Invalid("buffer exceeds device limit"));
        }
        let cpu_reservation = cpu_budget.try_reserve(output_bytes)?;
        let gpu_reservation =
            std::sync::Arc::new(gpu_budget.try_reserve(source_bytes + output_bytes * 2 + 64)?);
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        let mut result = cpu_reservation.try_buffer(source_values.len(), [0.0f32; 4])?;
        let source = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("managed Bayer demosaic input"),
            contents: bytemuck::cast_slice(source_values),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic output"),
            size: output_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("managed Bayer demosaic readback"),
            size: output_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let params: [u32; 12] = [
            w,
            h,
            0,
            h,
            cfa[0],
            cfa[1],
            cfa[2],
            cfa[3],
            0,
            0,
            algorithm.shader_code(),
            0,
        ];
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("managed Bayer demosaic uniform"),
            contents: bytemuck::cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
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
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_bytes);
        if cancelled() {
            return Err(BayerDemosaicError::Cancelled);
        }
        let submission = queue.submit([encoder.finish()]);
        let in_flight = gpu_reservation.clone();
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
            .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?
            .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
        let mapped = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|e| BayerDemosaicError::Readback(e.to_string()))?;
        let copied = (|| {
            for (dst, src) in result
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
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
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
    fn bayer_demosaic_compute_rejects_invalid_inputs_and_cancels_cleanly() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let compute = BayerDemosaicCompute::new(&device);
        let gpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let cpu_budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);

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
