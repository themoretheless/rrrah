//! Explicit color preparation for raster GPU input, separate from decoding.

use crate::raster::MAX_RASTER_BYTES;
use moxcms::{ColorProfile, DataColorSpace, Layout, ToneReprCurve, TransformOptions};
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterError, RasterPixels};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RasterColorError {
    #[error(transparent)]
    Frame(#[from] RasterError),
    #[error("ICC color transform failed: {0}")]
    Icc(String),
    #[error("ICC source must describe RGB or replicated grayscale samples; other layouts are unsupported")]
    UnsupportedProfileSpace,
    #[error("ICC input RGB samples must be finite and normalized to zero through one")]
    InvalidInput,
    #[error("linear raster exceeds the bounded allocation limit")]
    OutputTooLarge,
}

/// Return straight-alpha, linear sRGB floats for display. This uses relative
/// colorimetric ICC conversion, without gamut clipping or tone mapping.
/// Decoder pixels remain untouched; advisory unit scale remains metadata.
#[allow(clippy::cast_possible_truncation)] // GPU output is f32; bounds are checked before narrowing.
pub fn prepare_raster_for_display(frame: &DecodedRaster) -> Result<DecodedRaster, RasterColorError> {
    prepare_raster_for_display_with_budget(frame, None)
}

pub fn prepare_raster_for_display_with_budget(
    frame: &DecodedRaster,
    budget: Option<&rrrah_core::MemoryBudget>,
) -> Result<DecodedRaster, RasterColorError> {
    prepare_raster_for_display_with_budget_and_cancel(frame, budget, || false)
}

/// Check cancellation between setup stages and at most every 4096 pixels.
/// The profile parser and transform constructor are indivisible operations.
pub fn prepare_raster_for_display_with_budget_and_cancel(
    frame: &DecodedRaster,
    budget: Option<&rrrah_core::MemoryBudget>,
    cancel: impl Fn() -> bool,
) -> Result<DecodedRaster, RasterColorError> {
    let check = || -> Result<(), RasterColorError> {
        if cancel() {
            Err(RasterError::Cancelled.into())
        } else {
            Ok(())
        }
    };
    check()?;
    let count = usize::try_from(frame.width())
        .ok()
        .and_then(|w| {
            usize::try_from(frame.height())
                .ok()
                .and_then(|h| w.checked_mul(h))
        })
        .and_then(|n| n.checked_mul(4))
        .ok_or(RasterColorError::OutputTooLarge)?;
    if count as u64 > MAX_RASTER_BYTES / 4 {
        return Err(RasterColorError::OutputTooLarge);
    }
    let RasterColorSpace::Icc(bytes) = frame.color_space() else {
        return Ok(frame.to_linear_srgb_with_budget_and_cancel(budget, &cancel)?);
    };
    let mut profile =
        ColorProfile::new_from_slice(bytes).map_err(|e| RasterColorError::Icc(e.to_string()))?;
    check()?;
    let gray = profile.color_space == DataColorSpace::Gray;
    if profile.color_space != DataColorSpace::Rgb && !gray {
        return Err(RasterColorError::UnsupportedProfileSpace);
    }
    let sample = |index: usize| match frame.pixels() {
        RasterPixels::Rgba8(p) => f64::from(p[index]) / 255.0,
        RasterPixels::Rgba16(p) => f64::from(p[index]) / 65535.0,
        RasterPixels::Rgba32Float(p) => f64::from(p[index]),
    };
    if gray {
        for index in 0..count / 4 {
            if index.is_multiple_of(4096) {
                check()?;
            }
            let at = index * 4;
            if sample(at) != sample(at + 1) || sample(at) != sample(at + 2) {
                return Err(RasterColorError::UnsupportedProfileSpace);
            }
        }
    }
    // Reject inadmissible float input before reserving the full output and rows.
    // Integer storage is normalized by construction; float ICC input has this
    // explicit contract, unlike the separate scene-linear HDR path.
    if let RasterPixels::Rgba32Float(pixels) = frame.pixels() {
        for (index, rgba) in pixels.chunks_exact(4).enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if rgba[..3]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err(RasterColorError::InvalidInput);
            }
            if !rgba[3].is_finite() || !(0.0..=1.0).contains(&rgba[3]) {
                return Err(RasterError::InvalidAlpha.into());
            }
        }
    }
    // ICC curveType with one u8Fixed8 gamma is mathematically identical to
    // parametric type 0. Use its endpoint-safe evaluator: the extended-range
    // pure-gamma approximation in moxcms 0.8.1 is incorrect at exact zero.
    for curve in [
        &mut profile.red_trc,
        &mut profile.green_trc,
        &mut profile.blue_trc,
        &mut profile.gray_trc,
    ] {
        if let Some(ToneReprCurve::Lut(values)) = curve {
            if values.len() == 1 {
                *curve = Some(ToneReprCurve::Parametric(vec![f32::from(values[0]) / 256.0]));
            }
        }
    }
    let mut destination = ColorProfile::new_srgb();
    // The predefined CMS profile approximates D65 from 6504 K. The renderer
    // and native primary transforms use the standard xy coordinates instead.
    destination.update_rgb_colorimetry(
        moxcms::XyY::new(0.3127, 0.3290, 1.),
        moxcms::ColorPrimaries::BT_709,
    );
    let linear = ToneReprCurve::Parametric(vec![1.0]);
    destination.red_trc = Some(linear.clone());
    destination.green_trc = Some(linear.clone());
    destination.blue_trc = Some(linear);
    destination.cicp = None;
    let options = TransformOptions {
        rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
        prefer_fixed_point: false,
        allow_use_cicp_transfer: false,
        allow_extended_range_rgb_xyz: true,
        ..TransformOptions::default()
    };
    let transform = profile
        .create_transform_f64(
            if gray { Layout::Gray } else { Layout::Rgb },
            &destination,
            Layout::Rgb,
            options,
        )
        .map_err(|e| RasterColorError::Icc(e.to_string()))?;
    check()?;
    let reservation = budget
        .map(|b| b.try_reserve(count as u64 * 4))
        .transpose()
        .map_err(RasterError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| RasterColorError::OutputTooLarge)?;
    let width = frame.width() as usize;
    let row = |length| -> Result<rrrah_core::PixelBuffer<f64>, RasterColorError> {
        match budget {
            Some(budget) => budget
                .try_buffer(length, 0.0_f64)
                .map(|buffer| buffer.freeze().into())
                .map_err(|error| RasterError::Memory(error).into()),
            None => Ok(Arc::new(vec![0.0_f64; length]).into()),
        }
    };
    let channels = if gray { 1 } else { 3 };
    let mut source = row(width * channels)?;
    let mut target = row(width * 3)?;
    let source = source.get_mut().expect("exclusive conversion row");
    let target = target.get_mut().expect("exclusive conversion row");
    for y in 0..frame.height() as usize {
        for x in 0..width {
            if x.is_multiple_of(4096) {
                check()?;
            }
            for channel in 0..channels {
                let value = sample((y * width + x) * 4 + channel);
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(RasterColorError::InvalidInput);
                }
                source[x * channels + channel] = value;
            }
        }
        for (input, output) in source.chunks(4096 * channels).zip(target.chunks_mut(4096 * 3)) {
            check()?;
            transform
                .transform(input, output)
                .map_err(|e| RasterColorError::Icc(e.to_string()))?;
        }
        for x in 0..width {
            if x.is_multiple_of(4096) {
                check()?;
            }
            let alpha = sample((y * width + x) * 4 + 3);
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err(RasterError::InvalidAlpha.into());
            }
            let rgb = &target[x * 3..x * 3 + 3];
            if rgb
                .iter()
                .any(|v| !v.is_finite() || v.abs() > f64::from(f32::MAX))
            {
                return Err(RasterError::NonFiniteSample.into());
            }
            output.extend_from_slice(&[rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, alpha as f32]);
        }
    }
    check()?;
    Ok(DecodedRaster::new(
        frame.width(),
        frame.height(),
        RasterPixels::Rgba32Float(match reservation {
            Some(reservation) => reservation.try_adopt(output).map_err(RasterError::Memory)?.into(),
            None => Arc::new(output).into(),
        }),
        RasterColorSpace::LinearSrgb,
    )?
    .with_pixel_aspect(frame.pixel_aspect())?
    .with_sample_scale(frame.sample_scale())?
    .with_hotspot(frame.hotspot())?
    .with_image_selection(frame.image_index(), frame.image_count())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_float_icc_input_is_refused_before_output_memory_admission() {
        let icc = ColorProfile::new_srgb().encode().unwrap();
        for invalid in [-0.01, 1.01, 4.0] {
            let frame = DecodedRaster::new(
                2,
                1,
                RasterPixels::Rgba32Float(Arc::new(vec![0.5, 0.5, 0.5, 1.0, invalid, 0.5, 0.5, 1.0]).into()),
                RasterColorSpace::Icc(icc.clone()),
            )
            .unwrap();
            let budget = rrrah_core::MemoryBudget::new(0);
            assert!(matches!(
                prepare_raster_for_display_with_budget(&frame, Some(&budget)),
                Err(RasterColorError::InvalidInput)
            ));
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0);
        }
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![0.5, 0.5, 0.5, 1.0]).into()),
            RasterColorSpace::Icc(icc),
        )
        .unwrap();
        let polls = std::cell::Cell::new(0);
        let budget = rrrah_core::MemoryBudget::new(0);
        assert!(matches!(
            prepare_raster_for_display_with_budget_and_cancel(&frame, Some(&budget), || {
                polls.set(polls.get() + 1);
                polls.get() == 3
            }),
            Err(RasterColorError::Frame(RasterError::Cancelled))
        ));
        assert_eq!(budget.peak(), 0);
    }

    #[test]
    fn adjacent_sixteen_bit_samples_remain_distinct_after_icc_conversion() {
        let icc = ColorProfile::new_srgb().encode().unwrap();
        let frame = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba16(Arc::new(vec![32768, 32768, 32768, 0, 32769, 32769, 32769, 65535]).into()),
            RasterColorSpace::Icc(icc),
        )
        .unwrap();
        let result = prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = result.pixels() else {
            panic!("wrong precision")
        };
        assert!(p[4] > p[0]);
        assert_eq!(p[3], 0.0);
        assert_eq!(p[7], 1.0);
    }
    #[test]
    fn independent_littlecms_profile_matches_srgb_reference_across_the_pattern() {
        use image::ImageDecoder;
        let bytes = include_bytes!("../../../tests/fixtures/raster/pattern.profiled.png");
        let mut decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes.as_slice())).unwrap();
        let icc = decoder.icc_profile().unwrap().unwrap();
        let img = image::DynamicImage::from_decoder(decoder).unwrap().into_rgba8();
        let frame = DecodedRaster::new(
            img.width(),
            img.height(),
            RasterPixels::Rgba8(Arc::new(img.into_raw()).into()),
            RasterColorSpace::Icc(icc),
        )
        .unwrap()
        .with_image_selection(2, 5)
        .unwrap();
        let result = prepare_raster_for_display(&frame).unwrap();
        assert_eq!((result.image_index(), result.image_count()), (2, 5));
        let RasterPixels::Rgba32Float(actual) = result.pixels() else {
            panic!("wrong precision")
        };
        let reference = include_bytes!("../../../tests/fixtures/raster/pattern.profiled.png.rgba");
        let reference = DecodedRaster::new(
            16,
            16,
            RasterPixels::Rgba8(Arc::new(reference.to_vec()).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let RasterPixels::Rgba32Float(expected) = reference.pixels() else {
            panic!("wrong precision")
        };
        for (index, (&actual, &expected)) in actual.iter().zip(expected.iter()).enumerate() {
            let tolerance = if index % 4 == 3 { 1e-7 } else { 1.0 / 1024.0 };
            assert!(
                (actual - expected).abs() <= tolerance,
                "sample {index}: {actual} vs {expected}"
            );
        }
    }
    #[test]
    fn gray_icc_uses_gray_curve_and_refuses_nonreplicated_samples() {
        let icc = ColorProfile::new_gray_with_gamma(2.0).encode().unwrap();
        let make = |rgb: [f32; 3]| {
            DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba32Float(Arc::new(vec![rgb[0], rgb[1], rgb[2], 0.25]).into()),
                RasterColorSpace::Icc(icc.clone()),
            )
            .unwrap()
        };
        let budget = rrrah_core::MemoryBudget::new(1024);
        let ready = prepare_raster_for_display_with_budget(&make([0.5; 3]), Some(&budget)).unwrap();
        let RasterPixels::Rgba32Float(values) = ready.pixels() else {
            panic!()
        };
        for value in &values[..3] {
            assert!((*value - 0.25).abs() < 0.0001, "{value}");
        }
        assert_eq!(values[3], 0.25);
        drop(ready);
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            prepare_raster_for_display_with_budget(&make([0.5, 0.6, 0.5]), Some(&budget)),
            Err(RasterColorError::UnsupportedProfileSpace)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn gray_icc_png_alpha16_and_tiff16_reach_display_with_native_precision() {
        let profile = ColorProfile::new_gray_with_gamma(2.0).encode().unwrap();
        let samples = [0u16, 32768, 32769, 65535];
        let alpha = [0u16, 16384, 32768, 65535];
        for png in [true, false] {
            let mut rgba16 = Vec::new();
            for (index, &s) in samples.iter().enumerate() {
                rgba16.extend([s, s, s, if png { alpha[index] } else { 65535 }]);
            }
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let frame = DecodedRaster::new(
                4,
                1,
                RasterPixels::Rgba16(Arc::new(rgba16).into()),
                RasterColorSpace::Icc(profile.clone()),
            )
            .unwrap()
            .try_manage_pixels(&budget)
            .unwrap();
            let ready = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(linear) = ready.pixels() else {
                panic!()
            };
            for (index, pixel) in linear.chunks_exact(4).enumerate() {
                let expected = (f64::from(samples[index]) / 65535.).powi(2) as f32;
                for channel in &pixel[..3] {
                    assert!((*channel - expected).abs() < 0.0001);
                }
                let expected_alpha = if png { f32::from(alpha[index]) / 65535. } else { 1. };
                assert_eq!(pixel[3], expected_alpha);
            }
            assert!(linear[4] < linear[8], "adjacent 16-bit values merged");
            drop(ready);
            drop(frame);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn srgb_icc_converts_gray_without_gamma_correcting_alpha() {
        let icc = ColorProfile::new_srgb().encode().unwrap();
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba16(Arc::new(vec![32768, 32768, 32768, 16384]).into()),
            RasterColorSpace::Icc(icc),
        )
        .unwrap();
        let result = prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = result.pixels() else {
            panic!("wrong precision")
        };
        assert!((p[0] - 0.21405).abs() < 0.0002);
        assert!((p[3] - 16384.0 / 65535.0).abs() < 1e-7);
    }
    #[test]
    fn wide_gamut_red_is_not_clipped_and_invalid_profiles_fail() {
        let icc = ColorProfile::new_display_p3().encode().unwrap();
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(Arc::new(vec![255, 0, 0, 0]).into()),
            RasterColorSpace::Icc(icc),
        )
        .unwrap();
        let result = prepare_raster_for_display(&frame).unwrap();
        let RasterPixels::Rgba32Float(p) = result.pixels() else {
            panic!("wrong precision")
        };
        assert!(p[0] > 1.2, "{p:?}");
        assert!(p[1] < 0.0);
        assert_eq!(p[3], 0.0);
        let invalid = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba8(Arc::new(vec![0; 4]).into()),
            RasterColorSpace::Icc(vec![0; 128]),
        )
        .unwrap();
        assert!(matches!(
            prepare_raster_for_display(&invalid),
            Err(RasterColorError::Icc(_))
        ));
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn srgb_and_icc_output_admission_preserve_samples_and_last_owner() {
        for color in [
            RasterColorSpace::Srgb,
            RasterColorSpace::Icc(ColorProfile::new_srgb().encode().unwrap()),
        ] {
            let frame = DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba16(Arc::new(vec![32768, 32769, 65535, 12345]).into()),
                color,
            )
            .unwrap();
            let expected = prepare_raster_for_display(&frame).unwrap();
            let denied_limits: &[u64] = if matches!(frame.color_space(), RasterColorSpace::Icc(_)) {
                &[15, 16, 39, 40, 63]
            } else {
                &[15]
            };
            for &limit in denied_limits {
                let denied = rrrah_core::MemoryBudget::new(64);
                let held = denied.try_reserve(64 - limit).unwrap();
                assert!(matches!(
                    prepare_raster_for_display_with_budget(&frame, Some(&denied)),
                    Err(RasterColorError::Frame(RasterError::Memory(_)))
                ));
                assert_eq!(
                    denied.used(),
                    64 - limit,
                    "leaked reservation after refusal at {limit}"
                );
                // A later admitted conversion remains possible on this same budget.
                drop(held);
                let retry = prepare_raster_for_display_with_budget(&frame, Some(&denied)).unwrap();
                drop(retry);
                assert_eq!(denied.used(), 0);
            }
            let peak = if matches!(frame.color_space(), RasterColorSpace::Icc(_)) {
                64
            } else {
                16
            };
            let budget = rrrah_core::MemoryBudget::new(peak);
            let actual = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (actual.pixels(), expected.pixels())
            else {
                panic!("precision changed")
            };
            assert!(a.is_managed());
            assert_eq!(a.as_slice(), b.as_slice());
            assert_eq!(budget.peak(), peak);
            let retained = a.clone();
            drop(actual);
            assert_eq!(budget.used(), 16);
            drop(retained);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn managed_linear_hdr_reuses_pixels_without_another_reservation() {
        let budget = rrrah_core::MemoryBudget::new(16);
        let frame = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4.0, -0.5, 2.0, 1.0]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let prepared = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
            (frame.pixels(), prepared.pixels())
        else {
            panic!()
        };
        assert!(a.ptr_eq(b));
        assert_eq!(budget.peak(), 16);
        drop(frame);
        assert_eq!(budget.used(), 16);
        drop(prepared);
        assert_eq!(budget.used(), 0);
    }
}
