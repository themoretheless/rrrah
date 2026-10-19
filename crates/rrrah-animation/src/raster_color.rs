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
    #[error(
        "ICC input must be finite, nonnegative and normalized unless the source profile is explicitly linear"
    )]
    InvalidInput,
    #[error("linear raster exceeds the bounded allocation limit")]
    OutputTooLarge,
}

/// Return linear sRGB floats for display, retaining qualified alpha association. Uses relative
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
    // Managed callers admit every output allocation against their configured
    // budget; retain the legacy hard cap only when no budget was supplied.
    if count as u64 > MAX_RASTER_BYTES / 4 && budget.is_none() {
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
    // Signed finite HDR is defined for matrix/gray profiles whose input
    // curves are exactly linear. LUT profiles and nonlinear curves retain the
    // normalized-input contract; their extrapolation is not qualified.
    let linear_curve = |curve: &Option<ToneReprCurve>| match curve {
        Some(ToneReprCurve::Lut(values)) => values.is_empty() || values.as_slice() == [256],
        Some(ToneReprCurve::Parametric(values)) => values.as_slice() == [1.0],
        _ => false,
    };
    let extended_linear = profile.lut_a_to_b_perceptual.is_none()
        && profile.lut_a_to_b_colorimetric.is_none()
        && profile.lut_a_to_b_saturation.is_none()
        && if gray {
            linear_curve(&profile.gray_trc)
        } else {
            linear_curve(&profile.red_trc)
                && linear_curve(&profile.green_trc)
                && linear_curve(&profile.blue_trc)
        };
    let valid_input = |value: f64| value.is_finite() && (extended_linear || (0.0..=1.0).contains(&value));
    // Reject inadmissible float input before reserving the full output and rows.
    // Integer storage is normalized by construction. Float ICC input uses the
    // profile-dependent contract above; alpha is always normalized.
    if let RasterPixels::Rgba32Float(pixels) = frame.pixels() {
        for (index, rgba) in pixels.chunks_exact(4).enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if rgba[..3].iter().any(|v| !valid_input(f64::from(*v))) {
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
        // Gamma exactly one is identity on the signed extended domain. The
        // generic parametric evaluator clips negative operands; an empty ICC
        // curve selects the mathematically equivalent identity matrix path.
        if linear_curve(curve) {
            *curve = Some(ToneReprCurve::Lut(Vec::new()));
            continue;
        }
        if let Some(ToneReprCurve::Lut(values)) = curve {
            if values.len() == 1 {
                *curve = Some(ToneReprCurve::Parametric(vec![f32::from(values[0]) / 256.0]));
            }
        }
    }
    let mut destination = ColorProfile::new_srgb();
    rrrah_vector::icc_colorimetry::set_rgb_colorimetry(
        &mut destination,
        [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
    )
    .expect("standard sRGB chromaticities are valid");
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
                if !valid_input(value) {
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
    .with_animation_timing(frame.animation_timing())
    .with_image_selection(frame.image_index(), frame.image_count())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_linear_rgb_and_gray_match_independent_lcms_double_oracles() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for (profile, gray) in [
            ("linear-identity-rgb.icc", false),
            ("psd-tools-4x4_32bit_rgb.psd.icc", false),
            ("animation-linear-float.icc", false),
            ("linear-identity-grayscale.icc", true),
            ("psd-tools-4x4_32bit_grayscale.psd.icc", true),
        ] {
            let raw = std::fs::read(directory.join(if gray {
                "signed-linear-gray.rgba32fle"
            } else {
                "signed-linear-rgb.rgba32fle"
            }))
            .unwrap();
            let values: Vec<f32> = raw
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let count = values.len() / 4;
            let expected = std::fs::read(directory.join(format!("{profile}.signed-linear.rgb64le"))).unwrap();
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let frame = DecodedRaster::new(
                count as u32,
                1,
                RasterPixels::Rgba32Float(Arc::new(values.clone()).into()),
                RasterColorSpace::Icc(std::fs::read(directory.join(profile)).unwrap()),
            )
            .unwrap();
            let ready = prepare_raster_for_display_with_budget(&frame, Some(&root)).unwrap();
            let RasterPixels::Rgba32Float(actual) = ready.pixels() else {
                panic!()
            };
            let mut maximum = 0f64;
            for index in 0..count {
                for channel in 0..3 {
                    let offset = (index * 3 + channel) * 8;
                    let value = f64::from_le_bytes(expected[offset..offset + 8].try_into().unwrap());
                    let error = (f64::from(actual[index * 4 + channel]) - value).abs();
                    maximum = maximum.max(error);
                    assert!(
                        error <= 2e-5,
                        "{profile} pixel {index} channel {channel}: {error}"
                    );
                }
                assert_eq!(actual[index * 4 + 3].to_bits(), values[index * 4 + 3].to_bits());
            }
            eprintln!("SIGNED_LINEAR_ICC profile={profile} max_error={maximum} bound=0.00002");
            drop(ready);
            assert_eq!(root.used(), 0);
        }
    }
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
        let mut decoder =
            image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes.as_slice())).unwrap();
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
    fn original_gray_alpha_display_matches_independent_littlecms_all_pixels() {
        let pixels = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.rgba");
        let profile = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0.icc");
        let oracle = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray0-lcms.rgb64le");
        assert_eq!(oracle.len(), 256 * 3 * 8);
        let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
        let frame = DecodedRaster::new(
            400,
            359,
            RasterPixels::Rgba8(Arc::new(pixels.to_vec()).into()),
            RasterColorSpace::Icc(profile.to_vec()),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let prepared = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        let RasterPixels::Rgba32Float(actual) = prepared.pixels() else {
            panic!()
        };
        let mut maximum = 0f64;
        for (pixel, rgba) in pixels.chunks_exact(4).enumerate() {
            assert_eq!(rgba[0], rgba[1]);
            assert_eq!(rgba[1], rgba[2]);
            for c in 0..3 {
                let offset = (usize::from(rgba[0]) * 3 + c) * 8;
                let expected = f64::from_le_bytes(oracle[offset..offset + 8].try_into().unwrap());
                let error = (f64::from(actual[pixel * 4 + c]) - expected).abs();
                maximum = maximum.max(error);
                assert!(error <= 2e-5, "pixel={pixel} channel={c} error={error}");
            }
            assert_eq!(actual[pixel * 4 + 3], f32::from(rgba[3]) / 255.);
        }
        eprintln!("GRAY_ALPHA_LCMS pixels=143600 max_abs_error={maximum} alpha_exact=true");
        drop(prepared);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn linear_gray_icc_positive_hdr_matches_independent_littlecms() {
        for profile in [
            &include_bytes!("../../../tests/fixtures/raster/psd-tools-4x4_32bit_grayscale.psd.icc")[..],
            &include_bytes!("../../../tests/fixtures/raster/linear-identity-grayscale.icc")[..],
        ] {
            let expected = include_bytes!("../../../tests/fixtures/raster/psd-tools-gray-linear-hdr.rgb64le");
            let input = [1.00001f32, 2., 4., 16.];
            let values = input
                .into_iter()
                .enumerate()
                .flat_map(|(i, v)| [v, v, v, i as f32 / 4.])
                .collect::<Vec<_>>();
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let frame = DecodedRaster::new(
                4,
                1,
                RasterPixels::Rgba32Float(Arc::new(values).into()),
                RasterColorSpace::Icc(profile.to_vec()),
            )
            .unwrap()
            .try_manage_pixels(&budget)
            .unwrap();
            let ready = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(actual) = ready.pixels() else {
                panic!()
            };
            let mut maximum = 0f64;
            for pixel in 0..4 {
                for c in 0..3 {
                    let offset = (pixel * 3 + c) * 8;
                    let reference = f64::from_le_bytes(expected[offset..offset + 8].try_into().unwrap());
                    let error = (f64::from(actual[pixel * 4 + c]) - reference).abs();
                    maximum = maximum.max(error);
                    assert!(error <= 2e-5, "pixel={pixel} channel={c} error={error}");
                    assert!(actual[pixel * 4 + c] > 1.);
                }
                assert_eq!(actual[pixel * 4 + 3], pixel as f32 / 4.);
            }
            eprintln!("GRAY_LINEAR_HDR_LCMS max_abs_error={maximum} bound=0.00002 alpha_exact=true");
            drop(ready);
            drop(frame);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn linear_matrix_icc_preserves_extended_linear_range() {
        for bytes in [
            include_bytes!("../../../tests/fixtures/raster/psd-tools-4x4_32bit_rgb.psd.icc").to_vec(),
            include_bytes!("../../../tests/fixtures/raster/linear-identity-rgb.icc").to_vec(),
        ] {
            let expected = include_bytes!("../../../tests/fixtures/raster/psd-tools-linear-hdr.rgb64le");
            let make = |values: Vec<f32>| {
                DecodedRaster::new(
                    1,
                    1,
                    RasterPixels::Rgba32Float(Arc::new(values).into()),
                    RasterColorSpace::Icc(bytes.clone()),
                )
                .unwrap()
            };
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let frame = make(vec![1.00001, 2., 4., 0.25]);
            let ready = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(actual) = ready.pixels() else {
                panic!()
            };
            for (&value, expected) in actual[..3].iter().zip(
                expected
                    .chunks_exact(8)
                    .map(|b| f64::from_le_bytes(b.try_into().unwrap())),
            ) {
                assert!(
                    (f64::from(value) - expected).abs() <= 2e-5,
                    "{value} vs {expected}"
                );
            }
            assert_eq!(actual[3], 0.25);
            drop(ready);
            assert_eq!(budget.used(), 0);
            let signed = make(vec![-0.1, 2., 4., 0.25]);
            let ready = prepare_raster_for_display_with_budget(&signed, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(values) = ready.pixels() else {
                panic!()
            };
            assert!(values[0] < 0.0);
            drop(ready);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn gray_icc_matches_pinned_independent_littlecms_double_reference() {
        let profile = include_bytes!("../../../tests/fixtures/raster/gray-gamma2-lcms.icc");
        let reference = include_bytes!("../../../tests/fixtures/raster/gray-gamma2-lcms.rgb64le");
        let samples = [0u16, 32768, 32769, 65535];
        let alpha = [0u16, 16384, 32768, 65535];
        let values: Vec<u16> = samples
            .into_iter()
            .zip(alpha)
            .flat_map(|(value, alpha)| [value, value, value, alpha])
            .collect();
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let frame = DecodedRaster::new(
            4,
            1,
            RasterPixels::Rgba16(Arc::new(values).into()),
            RasterColorSpace::Icc(profile.to_vec()),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let prepared = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        let RasterPixels::Rgba32Float(actual) = prepared.pixels() else {
            panic!()
        };
        let expected: Vec<f64> = reference
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(expected.len(), 12);
        let mut maximum = 0f64;
        for pixel in 0..4 {
            for channel in 0..3 {
                maximum = maximum
                    .max((f64::from(actual[pixel * 4 + channel]) - expected[pixel * 3 + channel]).abs());
            }
            assert_eq!(actual[pixel * 4 + 3], f32::from(alpha[pixel]) / 65535.);
        }
        assert!(actual[4] < actual[8]);
        eprintln!(
            "GRAY_ICC_LCMS pixels=4 max_rgb_error={maximum:.12} bound=0.00002 alpha_exact=true adjacent_u16_distinct=true"
        );
        assert!(maximum <= 2e-5);
        drop(prepared);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn gray_icc_full_u16_range_matches_independent_littlecms() {
        let profile = include_bytes!("../../../tests/fixtures/raster/gray-gamma2-lcms.icc");
        let reference = include_bytes!("../../../tests/fixtures/raster/gray-gamma2-lcms-ramp.rgb64le");
        assert_eq!(reference.len(), 65536 * 3 * 8);
        let values: Vec<u16> = (0..=u16::MAX).flat_map(|v| [v, v, v, v]).collect();
        let native_bytes = 65536 * 8 + profile.len() as u64;
        let peak_bytes = native_bytes + 65536 * (16 + 8 + 24);
        let budget = rrrah_core::MemoryBudget::new(peak_bytes);
        let frame = DecodedRaster::new(
            65536,
            1,
            RasterPixels::Rgba16(Arc::new(values).into()),
            RasterColorSpace::Icc(profile.to_vec()),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let pressure = budget.try_reserve(1).unwrap();
        assert!(matches!(
            prepare_raster_for_display_with_budget(&frame, Some(&budget)),
            Err(RasterColorError::Frame(RasterError::Memory(_)))
        ));
        assert_eq!(budget.used(), native_bytes + 1);
        let RasterPixels::Rgba16(original) = frame.pixels() else {
            panic!()
        };
        for (code, rgba) in (0..=u16::MAX).zip(original.chunks_exact(4)) {
            assert_eq!(rgba, &[code; 4]);
        }
        drop(pressure);
        assert_eq!(budget.used(), native_bytes);
        let prepared = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
        assert_eq!(budget.peak(), peak_bytes);
        let RasterPixels::Rgba32Float(actual) = prepared.pixels() else {
            panic!()
        };
        let mut maximum = 0f64;
        for code in 0..=u16::MAX {
            let pixel = usize::from(code);
            for channel in 0..3 {
                let offset = (pixel * 3 + channel) * 8;
                let expected = f64::from_le_bytes(reference[offset..offset + 8].try_into().unwrap());
                let value = actual[pixel * 4 + channel];
                assert!(value.is_finite());
                maximum = maximum.max((f64::from(value) - expected).abs());
                if pixel > 0 {
                    assert!(
                        value > actual[(pixel - 1) * 4 + channel],
                        "code={code} channel={channel}"
                    );
                }
            }
            assert_eq!(actual[pixel * 4 + 3], f32::from(code) / 65535.);
        }
        assert!(maximum <= 2e-5, "max_rgb_error={maximum}");
        assert_eq!(budget.used(), 65536 * 24 + profile.len() as u64);
        eprintln!(
            "GRAY_ICC_FULL_U16 pixels=65536 max_rgb_error={maximum:.12} bound=0.00002 alpha_exact=true monotonic=true retained={}",
            budget.used()
        );
        drop(prepared);
        drop(frame);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn gray_icc_full_u16_cancellation_releases_rows_and_allows_retry() {
        use std::cell::Cell;
        let profile = include_bytes!("../../../tests/fixtures/raster/gray-gamma2-lcms.icc");
        let native_bytes = 65536 * 8 + profile.len() as u64;
        let peak_bytes = native_bytes + 65536 * (16 + 8 + 24);
        let budget = rrrah_core::MemoryBudget::new(peak_bytes);
        let values: Vec<u16> = (0..=u16::MAX).flat_map(|v| [v; 4]).collect();
        let frame = DecodedRaster::new(
            65536,
            1,
            RasterPixels::Rgba16(Arc::new(values).into()),
            RasterColorSpace::Icc(profile.to_vec()),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let checks = Cell::new(0usize);
        let prepared = prepare_raster_for_display_with_budget_and_cancel(&frame, Some(&budget), || {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
        let RasterPixels::Rgba32Float(expected) = prepared.pixels() else {
            panic!()
        };
        let expected = expected.to_vec();
        drop(prepared);
        assert!(checks.get() > 60);
        for stop in 1..=checks.get() {
            let visited = Cell::new(0usize);
            let result = prepare_raster_for_display_with_budget_and_cancel(&frame, Some(&budget), || {
                visited.set(visited.get() + 1);
                visited.get() == stop
            });
            assert!(
                matches!(result, Err(RasterColorError::Frame(RasterError::Cancelled))),
                "stop={stop}"
            );
            assert_eq!(visited.get(), stop);
            assert_eq!(budget.used(), native_bytes, "stop={stop}");
            let RasterPixels::Rgba16(original) = frame.pixels() else {
                panic!()
            };
            for (code, rgba) in (0..=u16::MAX).zip(original.chunks_exact(4)) {
                assert_eq!(rgba, &[code; 4]);
            }
            let retry = prepare_raster_for_display_with_budget(&frame, Some(&budget)).unwrap();
            let RasterPixels::Rgba32Float(actual) = retry.pixels() else {
                panic!()
            };
            assert_eq!(&actual[..], &expected[..], "stop={stop}");
            drop(retry);
            assert_eq!(budget.used(), native_bytes);
        }
        assert_eq!(budget.peak(), peak_bytes);
        eprintln!(
            "GRAY_ICC_CANCEL points={} pixels=65536 retry_bit_exact=true peak={peak_bytes}",
            checks.get()
        );
        drop(frame);
        assert_eq!(budget.used(), 0);
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
            .unwrap()
            .with_animation_timing(Some(rrrah_core::RasterAnimationTiming::new(1000, 60, 3).unwrap()));
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
            assert_eq!(actual.animation_timing(), frame.animation_timing());
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
