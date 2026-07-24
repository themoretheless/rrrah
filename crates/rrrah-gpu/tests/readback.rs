//! Pixel-level verification of the full color pipeline (RAW mosaic → tiled
//! upload → bilinear demosaic → WB/camera transform → ACES tone map → sRGB
//! target) through headless GPU readback.
//!
//! These tests skip (instead of failing) when no GPU adapter is available,
//! mirroring how the CR3 regression skips without `RRRAH_CR3_REGRESSION_DIR`.

// Test-side level/size computations use deliberately narrowing casts.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use common::{GpuReadback, cpu_reference_byte, pattern_mosaic, uniform_mosaic};

const FRAME: [u32; 2] = [256, 256];
const WHITE: f32 = 65_535.0;

/// Tolerance for GPU-vs-CPU-reference comparisons, in 8-bit codes. Covers the
/// f32 shader arithmetic vs the f64 reference, the hardware sRGB conversion,
/// and the unorm8 quantization rounding mode (round-to-nearest vs truncate).
const REFERENCE_TOLERANCE: u8 = 2;
/// Tolerance for channel-to-channel neutrality of a gray frame. All channels
/// run identical f32 math on identical inputs, so this is nearly exact.
const NEUTRAL_TOLERANCE: u8 = 1;

fn gpu() -> Option<GpuReadback> {
    let gpu = GpuReadback::new();
    if gpu.is_none() {
        eprintln!("readback: no GPU adapter available; skipping test");
    }
    gpu
}

#[test]
fn readback_is_bit_identical_across_rerenders() {
    let Some(gpu) = gpu() else { return };
    eprintln!("readback: adapter {}", gpu.adapter_name());
    // A non-uniform pattern exercises demosaic interpolation and tile edges;
    // determinism must hold bit-for-bit, not just approximately.
    let mosaic = pattern_mosaic(512, 384, WHITE, |x, y| {
        ((x.wrapping_mul(13).wrapping_add(y.wrapping_mul(7))) % 16_384) as u16
    });
    let first = gpu.render(&mosaic, FRAME);
    let second = gpu.render(&mosaic, FRAME);
    assert_eq!(first, second, "two renders of the same input must be bit-identical");
}

#[test]
fn uniform_gray_renders_neutral_and_spatially_uniform() {
    let Some(gpu) = gpu() else { return };
    for level_fraction in [0.25_f32, 0.5, 0.75] {
        let level = (WHITE * level_fraction) as u16;
        let frame = gpu.render(&uniform_mosaic(256, 256, level, WHITE), FRAME);
        let center = frame.center();
        assert_eq!(center[3], 255, "alpha must be opaque");
        let [r, g, b, _] = center;
        assert!(
            r.abs_diff(g) <= NEUTRAL_TOLERANCE && g.abs_diff(b) <= NEUTRAL_TOLERANCE,
            "gray input must stay neutral, got [{r}, {g}, {b}] at level {level}"
        );
        // Every fragment of a uniform field evaluates identical math, so the
        // whole frame matches the center almost exactly.
        let deviation = frame.max_channel_deviation(center);
        assert!(
            deviation <= NEUTRAL_TOLERANCE,
            "uniform input must render uniformly; max deviation {deviation} at level {level}"
        );
    }
}

#[test]
fn tone_curve_is_monotonic_across_input_levels() {
    let Some(gpu) = gpu() else { return };
    let mut previous = [0_u8; 3];
    for step in 0..=10_u32 {
        let level = (WHITE as u32 * step / 10) as u16;
        let frame = gpu.render(&uniform_mosaic(256, 256, level, WHITE), FRAME);
        let [r, g, b, _] = frame.center();
        let current = [r, g, b];
        for (channel, (current, previous)) in current.iter().zip(previous).enumerate() {
            assert!(
                *current >= previous,
                "channel {channel} regressed from {previous} to {current} at step {step}"
            );
        }
        previous = current;
    }
    // The endpoints must actually move: 0 -> black, white -> ACES shoulder.
    assert_eq!(previous[0], gpu.render(&uniform_mosaic(256, 256, WHITE as u16, WHITE), FRAME).center()[0]);
    assert!(previous[0] > 200, "white input must land on the ACES shoulder, got {previous:?}");
}

#[test]
fn black_and_white_endpoints_match_aces_contract() {
    let Some(gpu) = gpu() else { return };
    // Black maps to black: aces_fitted(0) = 0 -> sRGB 0.
    let black = gpu.render(&uniform_mosaic(256, 256, 0, WHITE), FRAME).center();
    assert_eq!(&black[..3], &[0, 0, 0], "black input must render black, got {black:?}");
    // White does NOT clip to 255: ACES(1.0) = 0.8038 (highlight roll-off),
    // sRGB(0.8038) ~= 0.908 -> ~232. This is the documented behavior of the
    // fitted curve at the top of the reference range.
    let white = gpu.render(&uniform_mosaic(256, 256, WHITE as u16, WHITE), FRAME).center();
    let expected = cpu_reference_byte(1.0);
    assert_eq!(expected, 232, "reference sanity: sRGB(ACES(1)) should be ~232");
    for (channel, (&actual, expected)) in white.iter().zip([expected; 3]).enumerate() {
        assert!(
            actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
            "channel {channel}: white maps to {actual}, reference {expected}"
        );
    }
}

#[test]
fn known_gray_points_match_cpu_reference_curve() {
    let Some(gpu) = gpu() else { return };
    // Reference points across the curve: deep shadow, 18% middle gray,
    // quarter/half/three-quarter tones, and full white.
    for normalized in [0.02_f64, 0.18, 0.25, 0.5, 0.75, 1.0] {
        let level = (f64::from(WHITE) * normalized) as u16;
        let frame = gpu.render(&uniform_mosaic(256, 256, level, WHITE), FRAME);
        let center = frame.center();
        let expected = cpu_reference_byte(normalized);
        for (channel, &actual) in center[..3].iter().enumerate() {
            assert!(
                actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
                "normalized {normalized}: channel {channel} readback {actual} vs reference {expected}"
            );
        }
    }
}

#[test]
fn exposure_shift_raises_midtones_without_replot() {
    let Some(gpu) = gpu() else { return };
    // +1 stop through the full GPU path must equal the reference curve
    // evaluated at twice the normalized level (pre-ACES, as in the shader).
    let mosaic = uniform_mosaic(256, 256, (WHITE * 0.18) as u16, WHITE);
    let (crop_width, crop_height) = mosaic.metadata.display_dimensions();
    let fit = ((FRAME[0] - 32) as f32 / crop_width as f32).min((FRAME[1] - 32) as f32 / crop_height as f32);
    let cover = (FRAME[0] as f32 / crop_width as f32).max(FRAME[1] as f32 / crop_height as f32);
    let view = rrrah_gpu::ViewParameters {
        viewport: [FRAME[0] as f32, FRAME[1] as f32],
        zoom: cover / fit,
        exposure_stops: 1.0,
        ..rrrah_gpu::ViewParameters::default()
    };
    let frame = gpu.render_with_view(&mosaic, view, FRAME);
    let expected = cpu_reference_byte(0.36);
    for (channel, &actual) in frame.center()[..3].iter().enumerate() {
        assert!(
            actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
            "+1 stop: channel {channel} readback {actual} vs reference {expected}"
        );
    }
}
