//! Linear HDR exposure without tone mapping; independent of display transfer.
//!
//! Core GPU pipeline and memory accounting live in [`gpu_kit::exposure`].
//! This module wraps them to attach `rrrah_core::RasterAlphaAssociation`.

pub use gpu_kit::exposure::ExposureError;

/// Queue-ordered linear RGBA32F result; storage remains admitted while any
/// owner or in-flight submission retains it. No CPU readback is performed.
#[derive(Debug, Clone)]
pub struct ResidentExposure {
    inner: gpu_kit::exposure::ResidentExposure,
    alpha_association: rrrah_core::RasterAlphaAssociation,
}

impl ResidentExposure {
    /// Exposure preserves RGB/alpha association; callers pass the input tag.
    pub fn with_alpha_association(mut self, mode: rrrah_core::RasterAlphaAssociation) -> Self {
        self.alpha_association = mode;
        self
    }

    pub fn alpha_association(&self) -> rrrah_core::RasterAlphaAssociation {
        self.alpha_association
    }

    pub(crate) fn raster_alpha_valid(&self) -> bool {
        self.inner.raster_alpha_valid()
    }

    /// Retain this owner or resource_lease through completion of downstream work.
    pub fn buffer(&self) -> &wgpu::Buffer {
        self.inner.buffer()
    }

    pub fn resource_lease(&self) -> crate::GpuResourceLease {
        self.inner.resource_lease()
    }

    pub fn pixels(&self) -> u32 {
        self.inner.pixels()
    }
}

impl From<gpu_kit::exposure::ResidentExposure> for ResidentExposure {
    fn from(inner: gpu_kit::exposure::ResidentExposure) -> Self {
        let alpha_association = if inner.alpha_associated() {
            rrrah_core::RasterAlphaAssociation::Associated
        } else {
            rrrah_core::RasterAlphaAssociation::Straight
        };
        Self {
            inner,
            alpha_association,
        }
    }
}

#[derive(Debug)]
pub struct LinearExposureCompute(gpu_kit::exposure::LinearExposureCompute);

impl LinearExposureCompute {
    pub fn new(device: &wgpu::Device) -> Self {
        Self(gpu_kit::exposure::LinearExposureCompute::new(device))
    }

    /// Expose prepared raster samples without losing RGB/alpha association.
    /// CPU ownership stays with the caller; output remains queue-ordered GPU data.
    pub fn execute_raster_resident<F: FnMut() -> bool>(
        &self,
        queue: &wgpu::Queue,
        raster: &rrrah_core::DecodedRaster,
        stops: f32,
        budget: &rrrah_core::MemoryBudget,
        cancelled: F,
    ) -> Result<ResidentExposure, ExposureError> {
        if raster.color_space() != &rrrah_core::RasterColorSpace::LinearSrgb {
            return Err(ExposureError::Invalid(
                "raster requires linear sRGB preparation",
            ));
        }
        let rrrah_core::RasterPixels::Rgba32Float(values) = raster.pixels() else {
            return Err(ExposureError::Invalid("raster requires float preparation"));
        };
        self.execute_resident(
            queue,
            bytemuck::cast_slice(values.as_ref()),
            stops,
            budget,
            cancelled,
        )
        .map(|output| output.with_alpha_association(raster.alpha_association()))
    }

    pub fn execute_resident<F: FnMut() -> bool>(
        &self,
        queue: &wgpu::Queue,
        samples: &[[f32; 4]],
        stops: f32,
        budget: &rrrah_core::MemoryBudget,
        cancelled: F,
    ) -> Result<ResidentExposure, ExposureError> {
        self.0
            .execute_resident(queue, samples, stops, budget, cancelled)
            .map(ResidentExposure::from)
    }

    pub fn execute_managed(
        &self,
        queue: &wgpu::Queue,
        source_pixels: &[[f32; 4]],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, ExposureError> {
        self.0
            .execute_managed(queue, source_pixels, stops, gpu_budget, cpu_budget)
    }

    pub fn execute_managed_with_cancel(
        &self,
        queue: &wgpu::Queue,
        source_pixels: &[[f32; 4]],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<[f32; 4]>, ExposureError> {
        self.0.execute_managed_with_cancel(
            queue,
            source_pixels,
            stops,
            gpu_budget,
            cpu_budget,
            cancelled,
        )
    }

    pub fn execute_interleaved_with_cancel(
        &self,
        queue: &wgpu::Queue,
        samples: &[f32],
        stops: f32,
        gpu_budget: &rrrah_core::MemoryBudget,
        cpu_budget: &rrrah_core::MemoryBudget,
        cancelled: impl FnMut() -> bool,
    ) -> Result<rrrah_core::PixelBuffer<f32>, ExposureError> {
        self.0
            .execute_interleaved_with_cancel(queue, samples, stops, gpu_budget, cpu_budget, cancelled)
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        output: &wgpu::Buffer,
        count: u32,
        stops: f32,
    ) -> Result<(), &'static str> {
        self.0.encode(encoder, source, output, count, stops)
    }
}
