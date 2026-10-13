//! Checked RGBE buffer and row-dispatch admission before GPU allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbeRowDispatch {
    pub first_row: u32,
    pub rows: u32,
    pub source_offset: u64,
    pub source_bytes: u64,
    pub source_first_sample: u32,
    pub output_offset: u64,
    pub output_bytes: u64,
    /// Global vec4 index corresponding to the aligned output binding start.
    pub output_first_pixel: u32,
    pub workgroups: [u32; 2],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbePlan {
    pub dimensions: [u32; 2],
    pub quad: [u32; 4],
    pub source_bytes: u64,
    pub output_bytes: u64,
    /// Input, output, readback and one 48-byte uniform per dispatch. Driver and
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
        if source_bytes > limits.max_buffer_size
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
        // Both binding offset and vec4 storage element alignment must hold.
        let window_alignment = alignment / gcd(alignment, 16) * 16;
        let mut dispatches = Vec::new();
        let mut first = 0u64;
        while first < u64::from(height) {
            let first_byte = first * row_bytes;
            let output_offset = first_byte / window_alignment * window_alignment;
            let prefix = first_byte - output_offset;
            let available = binding.saturating_sub(prefix) / row_bytes;
            let mut rows = available.min(max_rows).min(u64::from(height) - first);
            // Interpolation can read two neighboring rows in either direction.
            let input_row_bytes = u64::from(width) * 4;
            let source_start = first.saturating_sub(2) * input_row_bytes;
            let source_offset = source_start / window_alignment * window_alignment;
            let source_limit_row = (source_offset + binding) / input_row_bytes;
            if source_limit_row < u64::from(height) {
                rows = rows.min(source_limit_row.saturating_sub(first + 2));
            }
            let source_end = (first + rows + 2).min(u64::from(height)) * input_row_bytes;
            let dispatch_source_bytes = source_end - source_offset;
            if rows == 0 {
                return Err(fail("cannot fit sensor halo and output row in aligned binding windows"));
            }
            if dispatches.len() >= 4096 {
                return Err(fail("too many row dispatches"));
            }
            dispatches.push(RgbeRowDispatch {
                source_offset,
                source_bytes: dispatch_source_bytes,
                source_first_sample: (source_offset / 4) as u32,
                first_row: first as u32,
                rows: rows as u32,
                output_offset,
                output_bytes: prefix + rows * row_bytes,
                output_first_pixel: (output_offset / 16) as u32,
                workgroups: [xgroups, (rows as u32).div_ceil(8)],
            });
            first += rows;
        }
        let gpu_buffer_bytes = source_bytes + output_bytes * 2 + dispatches.len() as u64 * 48;
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
        assert_eq!(plan.gpu_buffer_bytes, 297561648);
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
    fn source_larger_than_binding_is_windowed_with_interpolation_halo() {
        let limits = wgpu::Limits { max_storage_buffer_binding_size: 1024, ..Default::default() };
        let plan = RgbePlan::new([17, 33], [3, 0, 2, 1], &limits).unwrap();
        assert!(plan.source_bytes > 1024);
        assert!(plan.dispatches.iter().any(|d| d.source_offset > 0));
        for d in &plan.dispatches {
            assert_eq!(d.source_offset % 256, 0);
            assert!(d.source_bytes <= 1024);
            assert!(d.source_offset <= u64::from(d.first_row.saturating_sub(2)) * 17 * 4);
            assert_eq!(d.source_offset + d.source_bytes,
                u64::from((d.first_row + d.rows + 2).min(33)) * 17 * 4);
        }
    }
    #[test]
    fn aligned_windows_contain_every_lattice_read_for_varied_odd_extents() {
        let mut accepted = 0;
        for width in 2..=65u32 {
            for height in 2..=65u32 {
                for binding in [256, 512, 1024, 4096] {
                    let limits = wgpu::Limits {
                        max_storage_buffer_binding_size: binding,
                        ..Default::default()
                    };
                    let Ok(plan) = RgbePlan::new([width, height], [3, 0, 2, 1], &limits) else {
                        continue;
                    };
                    accepted += 1;
                    let mut next = 0;
                    for d in &plan.dispatches {
                        assert_eq!(d.first_row, next);
                        assert!(d.rows > 0);
                        assert_eq!(d.source_offset % 256, 0);
                        assert_eq!(d.output_offset % 256, 0);
                        assert!(d.source_bytes <= u64::from(binding));
                        assert!(d.output_bytes <= u64::from(binding));
                        assert!(d.source_offset + d.source_bytes <= plan.source_bytes);
                        assert!(d.output_offset + d.output_bytes <= plan.output_bytes);
                        for y in d.first_row..d.first_row + d.rows {
                            // Independent nearest lattice rows: each CFA phase has a
                            // sample on every second row, clamped at sensor edges.
                            for phase in 0..2u32 {
                                let lattice: Vec<_> = (phase..height).step_by(2).collect();
                                let lower = lattice.iter().copied().filter(|v| *v <= y)
                                    .last().unwrap_or(lattice[0]);
                                let upper = lattice.iter().copied().find(|v| *v >= y)
                                    .unwrap_or(*lattice.last().unwrap());
                                for row in [lower, upper] {
                                    assert!(u64::from(row) * u64::from(width) * 4 >= d.source_offset);
                                    assert!(u64::from(row + 1) * u64::from(width) * 4
                                        <= d.source_offset + d.source_bytes);
                                }
                            }
                            let first_pixel = u64::from(y) * u64::from(width) * 16;
                            assert!(first_pixel >= d.output_offset);
                            assert!(first_pixel + u64::from(width) * 16
                                <= d.output_offset + d.output_bytes);
                        }
                        next += d.rows;
                    }
                    assert_eq!(next, height);
                    assert_eq!(plan.gpu_buffer_bytes,
                        plan.source_bytes + plan.output_bytes * 2 + plan.dispatches.len() as u64 * 48);
                }
            }
        }
        assert!(accepted >= 64 * 64, "all tested extents fit the 4096-byte cap");
        eprintln!("RGBE geometry sweep: attempted=16384; admitted={accepted}; every lattice read/output row in bounds");
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
        let plan = RgbePlan::new([17, 13], [3, 0, 2, 1], &small).unwrap();
        assert_eq!(plan.dispatches.len(), 5);
        let mut next_row = 0;
        for d in plan.dispatches {
            assert_eq!(d.first_row, next_row);
            assert_eq!(d.output_offset % 256, 0);
            assert_eq!(u64::from(d.output_first_pixel) * 16, d.output_offset);
            assert!(d.output_bytes <= 1024);
            assert_eq!(d.output_offset + d.output_bytes,
                u64::from(d.first_row + d.rows) * 17 * 16);
            next_row += d.rows;
        }
        assert_eq!(next_row, 13);
        let too_narrow = wgpu::Limits { max_storage_buffer_binding_size: 32, ..Default::default() };
        assert!(RgbePlan::new([3, 2], [3, 0, 2, 1], &too_narrow).is_err());
        assert!(RgbePlan::new([17, 2], [3, 0, 2, 1], &small).is_ok()); // One small complete binding needs no next aligned offset.
    }
}
