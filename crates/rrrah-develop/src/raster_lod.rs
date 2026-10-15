//! Managed scene-linear area filtering for bounded HDR levels of detail.
use crate::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterError, RasterPixels};

impl DecodedRaster {
    /// Area-filter straight-alpha linear RGBA32F into a smaller managed raster.
    /// Filter RGB in premultiplied space, then return straight alpha; invisible RGB
    /// cannot bleed into visible pixels. Signed/above-one HDR values are preserved.
    /// Fractional footprints cover the entire source, including odd edge extents.
    /// Pixel aspect is adjusted for rounded target dimensions to retain geometry.
    /// The source stays owned by its caller; only the output is charged here.
    pub fn downsample_linear_with_budget_and_cancel<F: FnMut() -> bool>(
        &self,
        target: [u32; 2],
        budget: &MemoryBudget,
        mut cancelled: F,
    ) -> Result<Self, RasterError> {
        if cancelled() {
            return Err(RasterError::Cancelled);
        }
        if target.contains(&0) {
            return Err(RasterError::Empty);
        }
        if target[0] > self.width() || target[1] > self.height() {
            return Err(RasterError::Dimensions);
        }
        let RasterPixels::Rgba32Float(input) = self.pixels() else {
            return Err(RasterError::ColorTransformRequired);
        };
        if self.color_space() != &RasterColorSpace::LinearSrgb {
            return Err(RasterError::ColorTransformRequired);
        }
        let count = (target[0] as usize)
            .checked_mul(target[1] as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or(RasterError::Dimensions)?;
        let mut output = budget.try_buffer(count, 0f32)?;
        let scale = [
            f64::from(self.width()) / f64::from(target[0]),
            f64::from(self.height()) / f64::from(target[1]),
        ];
        for oy in 0..target[1] {
            if cancelled() {
                return Err(RasterError::Cancelled);
            }
            let y0 = f64::from(oy) * scale[1];
            let y1 = (f64::from(oy + 1) * scale[1]).min(f64::from(self.height()));
            for ox in 0..target[0] {
                let x0 = f64::from(ox) * scale[0];
                let x1 = (f64::from(ox + 1) * scale[0]).min(f64::from(self.width()));
                let mut sum = [0f64; 4];
                for y in y0.floor() as u32..(y1.ceil() as u32).min(self.height()) {
                    if y.is_multiple_of(64) && cancelled() {
                        return Err(RasterError::Cancelled);
                    }
                    let wy = (y1.min(f64::from(y + 1)) - y0.max(f64::from(y))).max(0.);
                    for x in x0.floor() as u32..(x1.ceil() as u32).min(self.width()) {
                        if x.is_multiple_of(4096) && cancelled() {
                            return Err(RasterError::Cancelled);
                        }
                        let begin = (y as usize * self.width() as usize + x as usize) * 4;
                        let rgba = &input[begin..begin + 4];
                        if rgba.iter().any(|v| !v.is_finite()) {
                            return Err(RasterError::NonFiniteSample);
                        }
                        if !(0.0..=1.0).contains(&rgba[3]) {
                            return Err(RasterError::InvalidAlpha);
                        }
                        let wx = (x1.min(f64::from(x + 1)) - x0.max(f64::from(x))).max(0.);
                        let alpha = f64::from(rgba[3]) * wx * wy;
                        for c in 0..3 {
                            sum[c] += f64::from(rgba[c]) * alpha;
                        }
                        sum[3] += alpha;
                    }
                }
                let begin = (oy as usize * target[0] as usize + ox as usize) * 4;
                if sum[3] > 0. {
                    for c in 0..3 {
                        output[begin + c] = (sum[c] / sum[3]) as f32;
                    }
                }
                output[begin + 3] = (sum[3] / ((x1 - x0) * (y1 - y0))) as f32;
            }
        }
        if cancelled() {
            return Err(RasterError::Cancelled);
        }
        let aspect = (f64::from(self.pixel_aspect().unwrap_or(1.)) * scale[0] / scale[1]) as f32;
        let hotspot = self.hotspot().map(|(x, y)| {
            (
                (u64::from(x) * u64::from(target[0]) / u64::from(self.width())) as u32,
                (u64::from(y) * u64::from(target[1]) / u64::from(self.height())) as u32,
            )
        });
        DecodedRaster::new(
            target[0],
            target[1],
            RasterPixels::Rgba32Float(output.freeze().into()),
            RasterColorSpace::LinearSrgb,
        )?
        .with_pixel_aspect(Some(aspect))?
        .with_sample_scale(self.sample_scale())?
        .with_hotspot(hotspot)?
        .with_image_selection(self.image_index(), self.image_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn raster(width: u32, height: u32, pixels: Vec<f32>) -> DecodedRaster {
        DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
    }
    fn values(raster: &DecodedRaster) -> &[f32] {
        let RasterPixels::Rgba32Float(p) = raster.pixels() else {
            panic!()
        };
        p
    }
    #[test]
    fn transparent_rgb_does_not_bleed_and_hdr_stays_signed_and_bright() {
        let source = raster(
            2,
            2,
            vec![
                1000., -1000., 500., 0., -2., 8., 0.5, 1., -2., 8., 0.5, 0.5, 1000., -1000., 500., 0.,
            ],
        );
        let budget = MemoryBudget::new(16);
        let result = source
            .downsample_linear_with_budget_and_cancel([1, 1], &budget, || false)
            .unwrap();
        assert_eq!(values(&result), [-2., 8., 0.5, 0.375]);
        assert_eq!(budget.used(), 16);
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn odd_fractional_area_uses_both_edges_and_preserves_physical_aspect() {
        let source = raster(3, 1, vec![0., -3., 9., 1., 3., 0., 6., 1., 9., 6., 0., 1.])
            .with_pixel_aspect(Some(2.))
            .unwrap()
            .with_hotspot(Some((2, 0)))
            .unwrap()
            .with_image_selection(2, 4)
            .unwrap();
        let budget = MemoryBudget::new(32);
        let result = source
            .downsample_linear_with_budget_and_cancel([2, 1], &budget, || false)
            .unwrap();
        assert_eq!(values(&result), [1., -2., 8., 1., 7., 4., 2., 1.]);
        assert_eq!(result.pixel_aspect(), Some(3.));
        assert_eq!(
            (result.image_index(), result.image_count(), result.hotspot()),
            (2, 4, Some((1, 0)))
        );
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn rejection_and_mid_filter_cancellation_release_exact_output_credit() {
        let source = raster(128, 128, vec![1.; 128 * 128 * 4]);
        let short = MemoryBudget::new(15);
        assert!(matches!(
            source.downsample_linear_with_budget_and_cancel([1, 1], &short, || false),
            Err(RasterError::Memory(_))
        ));
        assert_eq!(short.used(), 0);
        let budget = MemoryBudget::new(16);
        let mut checkpoints = 0;
        assert!(matches!(
            source.downsample_linear_with_budget_and_cancel([1, 1], &budget, || {
                checkpoints += 1;
                checkpoints >= 6
            }),
            Err(RasterError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            source.downsample_linear_with_budget_and_cancel([0, 1], &budget, || false),
            Err(RasterError::Empty)
        ));
        assert!(matches!(
            source.downsample_linear_with_budget_and_cancel([129, 1], &budget, || false),
            Err(RasterError::Dimensions)
        ));
        for (component, value, alpha) in [(0, f32::NAN, false), (3, 2., true)] {
            let mut pixels = vec![1.; 16];
            pixels[component] = value;
            let bad = raster(2, 2, pixels);
            let result = bad.downsample_linear_with_budget_and_cancel([1, 1], &budget, || false);
            if alpha {
                assert!(matches!(result, Err(RasterError::InvalidAlpha)));
            } else {
                assert!(matches!(result, Err(RasterError::NonFiniteSample)));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn fractional_area_matches_independent_integer_overlap_oracle() {
        for width in 2..=9u32 {
            for height in 2..=7u32 {
                let target = [width - 1, (height + 1) / 2];
                let input: Vec<f32> = (0..width * height)
                    .flat_map(|i| {
                        [
                            (i % 13) as f32 / 4. - 1.,
                            (i % 7) as f32 / 2.,
                            -0.5,
                            [0., 0.25, 0.5, 1.][(i % 4) as usize],
                        ]
                    })
                    .collect();
                let source = raster(width, height, input.clone());
                let bytes = u64::from(target[0]) * u64::from(target[1]) * 16;
                let budget = MemoryBudget::new(bytes);
                let output = source
                    .downsample_linear_with_budget_and_cancel(target, &budget, || false)
                    .unwrap();
                for oy in 0..target[1] {
                    for ox in 0..target[0] {
                        let mut sum = [0f64; 4];
                        // Integer overlap lengths in destination-denominator units;
                        // no floating source-coordinate calculations from production.
                        for y in 0..height {
                            for x in 0..width {
                                let wx = ((x + 1) * target[0])
                                    .min((ox + 1) * width)
                                    .saturating_sub((x * target[0]).max(ox * width));
                                let wy = ((y + 1) * target[1])
                                    .min((oy + 1) * height)
                                    .saturating_sub((y * target[1]).max(oy * height));
                                let rgba = &input[((y * width + x) * 4) as usize..][..4];
                                let weight = f64::from(wx) * f64::from(wy) * f64::from(rgba[3]);
                                for c in 0..3 {
                                    sum[c] += f64::from(rgba[c]) * weight;
                                }
                                sum[3] += weight;
                            }
                        }
                        let alpha = sum[3] / f64::from(width * height);
                        let expected = if sum[3] > 0. {
                            [sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3], alpha]
                        } else {
                            [0.; 4]
                        };
                        let actual = &values(&output)[((oy * target[0] + ox) * 4) as usize..][..4];
                        for c in 0..4 {
                            assert!(
                                (f64::from(actual[c]) - expected[c]).abs() < 1e-6,
                                "source={width}x{height} output={ox},{oy} channel={c}"
                            );
                        }
                    }
                }
                assert_eq!(budget.used(), bytes);
                drop(output);
                assert_eq!(budget.used(), 0);
            }
        }
    }
}
