//! One active GPU histogram plus one replaceable input; only 1100 bytes read back.
//!
//! Core GPU histogram pipeline lives in [`gpu_kit::histogram`]; this module
//! connects it to `ResidentLinearBayer` and `rrrah_core::Rect`.

pub use gpu_kit::histogram::HISTOGRAM_JOB_BYTES;
use gpu_kit::histogram::HistogramRect;
use rrrah_core::{MemoryBudget, histogram::Histogram};
use std::sync::Arc;

type Cancel = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Debug)]
pub struct GpuHistogram(gpu_kit::histogram::GpuHistogram);

impl GpuHistogram {
    pub fn new(
        device: &wgpu::Device,
        budget: MemoryBudget,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self(gpu_kit::histogram::GpuHistogram::new(device, budget, wake))
    }

    /// Calibrated linear sRGB samples only, sampled inside the active crop.
    pub fn submit_bayer(
        &mut self,
        queue: &wgpu::Queue,
        input: &crate::ResidentLinearBayer,
        cancelled: Cancel,
    ) -> Result<(), String> {
        let crop = HistogramRect {
            x: input.crop.x,
            y: input.crop.y,
            width: input.crop.width,
            height: input.crop.height,
        };
        self.0.submit_buffer(
            queue,
            input.buffer().clone(),
            input.dimensions(),
            crop,
            input.resource_lease(),
            cancelled,
        )
    }

    pub fn poll(&mut self, queue: &wgpu::Queue) -> Option<Result<Histogram, String>> {
        self.0.poll(queue)
    }

    pub fn is_pending(&self) -> bool {
        self.0.is_pending()
    }

    pub fn cancel_pending(&mut self) {
        self.0.cancel_pending();
    }
}
