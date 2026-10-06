//! Owned CUDA PTX exposure kernel, separate from rendering API selection.
//! CUDA Driver API is loaded explicitly on Linux/Windows; no CPU fallback.
//! Accounting covers device input/output and retained CPU output, excluding
//! driver context/JIT overhead. Submitted work finishes before output is returned.
#[allow(unsafe_code)] // FFI is confined to the documented driver boundary.
mod driver;
mod readback;
use rrrah_memory::{MemoryBudget, SharedBuffer};

#[derive(Debug, thiserror::Error)]
pub enum CudaError {
    #[error("CUDA requires a 64-bit Linux or Windows host and an NVIDIA driver")]
    UnsupportedPlatform,
    #[error("CUDA driver unavailable: {0}")]
    Driver(String),
    #[error("CUDA operation {operation} failed with status {status}")]
    Operation { operation: &'static str, status: i32 },
    #[error("invalid CUDA input: {0}")]
    Invalid(&'static str),
    #[error("CUDA work cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_memory::BufferError),
}

#[derive(Debug)]
pub struct CudaExposure {
    driver: std::sync::Arc<driver::Driver>,
    ordinal: i32,
}
impl CudaExposure {
    /// Loads the NVIDIA driver; device ordinal is validated when executing.
    /// # Errors
    /// Rejects negative ordinals, unsupported hosts and unavailable drivers.
    pub fn new(ordinal: i32) -> Result<Self, CudaError> {
        if ordinal < 0 {
            return Err(CudaError::Invalid("negative device ordinal"));
        }
        Ok(Self {
            driver: driver::Driver::open()?,
            ordinal,
        })
    }
    /// Source CPU accounting belongs to the caller. Cancellation after launch
    /// discards output only after synchronization; device leases remain live.
    /// # Errors
    /// Reports invalid/nonfinite input, cancellation, memory admission failure
    /// or an explicit CUDA device, JIT, launch, transfer or cleanup failure.
    pub fn execute(
        &self,
        pixels: &[[f32; 4]],
        stops: f32,
        gpu: &MemoryBudget,
        cpu: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<SharedBuffer<[f32; 4]>, CudaError> {
        validate(pixels, stops, &mut cancelled)?;
        if pixels.is_empty() {
            return Ok(cpu.try_buffer(0, [0.; 4])?.freeze());
        }
        let bytes = pixels.len() as u64 * 16;
        let cpu_reservation = cpu.try_reserve(bytes)?;
        let reservation = gpu.try_reserve(bytes * 2)?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        let mut result = cpu_reservation.try_buffer(pixels.len(), [0.; 4])?;
        self.driver.expose(
            self.ordinal,
            pixels,
            stops.exp2(),
            &mut result,
            reservation,
            &mut cancelled,
        )?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        Ok(result.freeze())
    }
    /// Interleaved RGBA32F output for raster upload, without a flattening copy.
    /// Source CPU accounting belongs to the caller.
    /// # Errors
    /// Refuses incomplete pixels and the same errors as `execute`.
    pub fn execute_interleaved(
        &self,
        pixels: &[f32],
        stops: f32,
        gpu: &MemoryBudget,
        cpu: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<SharedBuffer<f32>, CudaError> {
        let (rgba, remainder) = pixels.as_chunks::<4>();
        if !remainder.is_empty() {
            return Err(CudaError::Invalid("incomplete interleaved RGBA pixel"));
        }
        validate(rgba, stops, &mut cancelled)?;
        if rgba.is_empty() {
            return Ok(cpu.try_buffer(0, 0f32)?.freeze());
        }
        let bytes = pixels.len() as u64 * 4;
        let cpu_reservation = cpu.try_reserve(bytes)?;
        let reservation = gpu.try_reserve(bytes * 2)?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        let mut result = cpu_reservation.try_buffer(pixels.len(), 0f32)?;
        self.driver.expose(
            self.ordinal,
            rgba,
            stops.exp2(),
            result.as_chunks_mut::<4>().0,
            reservation,
            &mut cancelled,
        )?;
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        Ok(result.freeze())
    }
}
fn validate(pixels: &[[f32; 4]], stops: f32, cancelled: &mut impl FnMut() -> bool) -> Result<(), CudaError> {
    if cancelled() {
        return Err(CudaError::Cancelled);
    }
    if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
        return Err(CudaError::Invalid("exposure outside finite -32..32 range"));
    }
    u32::try_from(pixels.len()).map_err(|_| CudaError::Invalid("too many pixels"))?;
    let gain = stops.exp2();
    for (index, pixel) in pixels.iter().enumerate() {
        if index % 4096 == 0 && cancelled() {
            return Err(CudaError::Cancelled);
        }
        if pixel.iter().any(|sample| !sample.is_finite())
            || pixel[..3].iter().any(|sample| !(sample * gain).is_finite())
        {
            return Err(CudaError::Invalid("nonfinite input or exposed RGB"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_hdr_and_alpha_are_accepted_but_nonfinite_and_overflow_refused() {
        assert!(validate(&[[-2., 8., 0.25, 0.5]], 1., &mut || false).is_ok());
        for pixel in [
            [f32::NAN, 0., 0., 1.],
            [0., 0., 0., f32::INFINITY],
            [f32::MAX, 0., 0., 1.],
        ] {
            assert!(validate(&[pixel], 1., &mut || false).is_err());
        }
        for stops in [f32::NAN, f32::INFINITY, -33., 33.] {
            assert!(validate(&[], stops, &mut || false).is_err());
        }
        assert!(matches!(
            validate(&[], 0., &mut || true),
            Err(CudaError::Cancelled)
        ));
    }
    #[test]
    fn unsupported_host_reports_cuda_unavailable_without_fallback() {
        #[cfg(not(all(target_pointer_width = "64", any(target_os = "linux", target_os = "windows"))))]
        assert!(matches!(
            CudaExposure::new(0),
            Err(CudaError::UnsupportedPlatform)
        ));
        assert!(matches!(CudaExposure::new(-1), Err(CudaError::Invalid(_))));
    }
    #[test]
    #[ignore = "requires CUDA-capable NVIDIA driver on 64-bit Linux/Windows; never a CPU fallback"]
    fn nvidia_full_output_boundaries_hdr_alpha_and_managed_limits() {
        let cuda = CudaExposure::new(0).expect("CUDA device required");
        for count in [1u32, 255, 256, 257, 1023, 1025, 65537] {
            let pixels: Vec<_> = (0..count)
                .map(|i| {
                    [
                        f32::from(u16::try_from(i % 257).unwrap()) - 128.,
                        f32::from(u16::try_from(i % 67).unwrap()) / 8.,
                        f32::from(u16::try_from(i % 101).unwrap()) / -4.,
                        f32::from(u16::try_from(i % 17).unwrap()) / 16.,
                    ]
                })
                .collect();
            for stops in [-2.5f32, 0., 1.25] {
                let cpu = MemoryBudget::new(u64::from(count) * 16);
                let gpu = MemoryBudget::new(u64::from(count) * 32);
                let result = cuda.execute(&pixels, stops, &gpu, &cpu, || false).unwrap();
                for (actual, expected) in result.iter().zip(&pixels) {
                    for channel in 0..3 {
                        assert_eq!(
                            actual[channel].to_bits(),
                            (expected[channel] * stops.exp2()).to_bits()
                        );
                    }
                    assert_eq!(actual[3].to_bits(), expected[3].to_bits());
                }
                assert_eq!(gpu.used(), 0);
                assert_eq!(gpu.peak(), u64::from(count) * 32);
                assert_eq!(cpu.used(), u64::from(count) * 16);
                let alias = result.clone();
                drop(result);
                assert_eq!(cpu.used(), u64::from(count) * 16);
                drop(alias);
                assert_eq!(cpu.used(), 0);
                let flat = cuda
                    .execute_interleaved(pixels.as_flattened(), stops, &gpu, &cpu, || false)
                    .unwrap();
                for (actual, expected) in flat.as_chunks::<4>().0.iter().zip(&pixels) {
                    for channel in 0..3 {
                        assert_eq!(
                            actual[channel].to_bits(),
                            (expected[channel] * stops.exp2()).to_bits()
                        );
                    }
                    assert_eq!(actual[3].to_bits(), expected[3].to_bits());
                }
                assert_eq!(gpu.used(), 0);
                drop(flat);
                assert_eq!(cpu.used(), 0);
            }
        }
        let cpu = MemoryBudget::new(16);
        let gpu = MemoryBudget::new(31);
        assert!(matches!(
            cuda.execute_interleaved(&[1.; 3], 0., &gpu, &cpu, || false),
            Err(CudaError::Invalid(_))
        ));
        assert_eq!(cpu.peak(), 0);
        assert_eq!(gpu.peak(), 0);
        assert!(matches!(
            cuda.execute(&[[1.; 4]], 0., &gpu, &cpu, || false),
            Err(CudaError::Memory(_))
        ));
        assert_eq!(cpu.used(), 0);
        assert_eq!(gpu.used(), 0);
        assert!(matches!(
            cuda.execute(&[[1.; 4]], 0., &gpu, &cpu, || true),
            Err(CudaError::Cancelled)
        ));
        assert_eq!(cpu.used(), 0);
    }
}
