//! Checked RGBE buffer and row-dispatch admission before GPU allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbeRowDispatch {
    pub first_row: u32,
    pub rows: u32,
    pub output_offset: u64,
    pub output_bytes: u64,
    pub workgroups: [u32; 2],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbePlan {
    pub dimensions: [u32; 2],
    pub quad: [u32; 4],
    pub source_bytes: u64,
    pub output_bytes: u64,
    /// Input, output, readback and one 32-byte uniform per dispatch. Driver and
    /// pipeline allocations are outside this explicit buffer accounting.
    pub gpu_buffer_bytes: u64,
    pub dispatches: Vec<RgbeRowDispatch>,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid RGBE compute request: {0}")]
pub struct RgbePlanError(pub &'static str);
impl RgbePlan {
    pub fn new(dimensions: [u32; 2], quad: [u32; 4], limits: &wgpu::Limits) -> Result<Self, RgbePlanError> {
        let fail = |s| RgbePlanError(s);
        let [width, height] = dimensions;
        if width < 2 || height < 2 {
            return Err(fail("sensor must contain all four planes"));
        }
        let mut seen = [false; 4];
        for c in quad {
            let flag = seen.get_mut(c as usize).ok_or_else(|| fail("unknown plane"))?;
            if *flag {
                return Err(fail("duplicate plane"));
            }
            *flag = true;
        }
        let count = width
            .checked_mul(height)
            .ok_or_else(|| fail("shader indexing overflow"))?;
        let source_bytes = u64::from(count) * 4;
        let output_bytes = u64::from(count) * 16;
        if source_bytes > u64::from(limits.max_storage_buffer_binding_size)
            || source_bytes > limits.max_buffer_size
            || output_bytes > limits.max_buffer_size
        {
            return Err(fail("sensor buffers exceed device limits"));
        }
        let xgroups = width.div_ceil(8);
        if limits.max_compute_invocations_per_workgroup < 64
            || limits.max_compute_workgroup_size_x < 8
            || limits.max_compute_workgroup_size_y < 8
            || xgroups > limits.max_compute_workgroups_per_dimension
        {
            return Err(fail("workgroup dimensions exceed device limits"));
        }
        let max_rows = u64::from(limits.max_compute_workgroups_per_dimension) * 8;
        let row_bytes = u64::from(width) * 16;
        let binding = u64::from(limits.max_storage_buffer_binding_size);
        let alignment = u64::from(limits.min_storage_buffer_offset_alignment);
        if alignment == 0 {
            return Err(fail("invalid storage alignment"));
        }
        fn gcd(mut a: u64, mut b: u64) -> u64 {
            while b != 0 {
                let r = a % b;
                a = b;
                b = r;
            }
            a
        }
        let row_alignment = alignment / gcd(row_bytes, alignment);
        let available = (binding / row_bytes).min(max_rows);
        let chunk = if available >= u64::from(height) {
            u64::from(height)
        } else {
            available / row_alignment * row_alignment
        };
        if chunk == 0 {
            return Err(fail("cannot bind an aligned row chunk"));
        }
        let dispatch_count = u64::from(height).div_ceil(chunk);
        if dispatch_count > 4096 {
            return Err(fail("too many row dispatches"));
        }
        let gpu_buffer_bytes = source_bytes + output_bytes * 2 + dispatch_count * 32;
        let mut dispatches = Vec::with_capacity(dispatch_count as usize);
        let mut first = 0u64;
        while first < u64::from(height) {
            let rows = chunk.min(u64::from(height) - first);
            dispatches.push(RgbeRowDispatch {
                first_row: first as u32,
                rows: rows as u32,
                output_offset: first * row_bytes,
                output_bytes: rows * row_bytes,
                workgroups: [xgroups, (rows as u32).div_ceil(8)],
            });
            first += rows;
        }
        Ok(Self {
            dimensions,
            quad,
            source_bytes,
            output_bytes,
            gpu_buffer_bytes,
            dispatches,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_srf_plan_and_small_limit_segments_cover_without_overlap() {
        let plan = RgbePlan::new([3360, 2460], [3, 0, 2, 1], &wgpu::Limits::default()).unwrap();
        assert_eq!(plan.source_bytes, 33062400);
        assert_eq!(plan.output_bytes, 132249600);
        assert_eq!(plan.gpu_buffer_bytes, 297561632);
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..Default::default()
        };
        let plan = RgbePlan::new([16, 13], [3, 0, 2, 1], &limits).unwrap();
        assert_eq!(
            plan.dispatches.iter().map(|d| d.rows).collect::<Vec<_>>(),
            [4, 4, 4, 1]
        );
        let mut end = 0;
        for d in plan.dispatches {
            assert_eq!(d.output_offset, end);
            assert_eq!(d.output_offset % 256, 0);
            assert!(d.output_bytes <= 1024);
            end += d.output_bytes;
        }
        assert_eq!(end, plan.output_bytes);
    }
    #[test]
    fn invalid_geometry_planes_and_device_admission_fail_before_allocating_pixels() {
        let limits = wgpu::Limits::default();
        for dims in [[1, 2], [2, 0], [u32::MAX, 2], [20000, 20000]] {
            assert!(RgbePlan::new(dims, [3, 0, 2, 1], &limits).is_err());
        }
        for quad in [[0, 1, 1, 2], [0, 1, 2, 255]] {
            assert!(RgbePlan::new([2, 2], quad, &limits).is_err());
        }
        let small = wgpu::Limits {
            max_storage_buffer_binding_size: 1024,
            ..Default::default()
        };
        assert!(RgbePlan::new([17, 13], [3, 0, 2, 1], &small).is_err()); // No aligned output-row chunk fits.
        assert!(RgbePlan::new([17, 2], [3, 0, 2, 1], &small).is_ok()); // One small complete binding needs no next aligned offset.
    }
}
