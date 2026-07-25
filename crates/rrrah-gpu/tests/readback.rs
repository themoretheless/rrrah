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

use common::{GpuReadback, cpu_reference_byte, cpu_reference_rgb, pattern_mosaic, profiled_pattern_mosaic, uniform_mosaic};

const FRAME: [u32; 2] = [256, 256];
const WHITE: f32 = 65_535.0;

/// Tolerance for GPU-vs-CPU-reference comparisons, in 8-bit codes. Covers the
/// f32 shader arithmetic vs the f64 reference, the hardware sRGB conversion,
/// and the unorm8 quantization rounding mode (round-to-nearest vs truncate).
const REFERENCE_TOLERANCE: u8 = 2;
/// Tolerance for channel-to-channel neutrality of a gray frame. All channels
/// run identical f32 math on identical inputs, so this is nearly exact.
const NEUTRAL_TOLERANCE: u8 = 1;
const EOS_R8_GAINS: [f32; 4] = [1_678.0 / 1_024.0, 1.0, 1_659.0 / 1_024.0, 1.0];
const EOS_R8_XYZ_TO_CAMERA: [[f32; 3]; 4] = [
    [0.9539, -0.2795, -0.1224],
    [-0.4175, 1.1998, 0.2458],
    [-0.0465, 0.1755, 0.6048],
    [0.0; 3],
];

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
    assert_eq!(
        first, second,
        "two renders of the same input must be bit-identical"
    );
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
    assert_eq!(
        previous[0],
        gpu.render(&uniform_mosaic(256, 256, WHITE as u16, WHITE), FRAME)
            .center()[0]
    );
    assert!(
        previous[0] > 200,
        "white input must land on the ACES shoulder, got {previous:?}"
    );
}

#[test]
fn black_and_white_endpoints_match_aces_contract() {
    let Some(gpu) = gpu() else { return };
    // Black maps to black: aces_fitted(0) = 0 -> sRGB 0.
    let black = gpu.render(&uniform_mosaic(256, 256, 0, WHITE), FRAME).center();
    assert_eq!(
        &black[..3],
        &[0, 0, 0],
        "black input must render black, got {black:?}"
    );
    // White does NOT clip to 255: ACES(1.0) = 0.8038 (highlight roll-off),
    // sRGB(0.8038) ~= 0.908 -> ~232. This is the documented behavior of the
    // fitted curve at the top of the reference range.
    let white = gpu
        .render(&uniform_mosaic(256, 256, WHITE as u16, WHITE), FRAME)
        .center();
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
fn decoder_white_balance_scale_is_preserved_through_real_camera_profile() {
    const SIZE: u32 = 256;
    const NEUTRAL: f32 = 0.18;
    let Some(gpu) = gpu() else { return };

    let direct_samples = [
        NEUTRAL / EOS_R8_GAINS[0],
        NEUTRAL / EOS_R8_GAINS[1],
        NEUTRAL / EOS_R8_GAINS[3],
        NEUTRAL / EOS_R8_GAINS[2],
    ]
    .map(|value| (value * WHITE).round() as u16);
    let unit_sample = (NEUTRAL * WHITE).round() as u16;
    let phase_sample = |samples: [u16; 4], x: u32, y: u32| {
        let phase = ((y & 1) * 2 + (x & 1)) as usize;
        samples[phase]
    };
    let unit = profiled_pattern_mosaic(SIZE, SIZE, WHITE, [1.0; 4], EOS_R8_XYZ_TO_CAMERA, |_x, _y| {
        unit_sample
    });
    let direct = profiled_pattern_mosaic(SIZE, SIZE, WHITE, EOS_R8_GAINS, EOS_R8_XYZ_TO_CAMERA, |x, y| {
        phase_sample(direct_samples, x, y)
    });
    let normalized_rgb =
        rrrah_core::luminance_normalize_wb_gains([EOS_R8_GAINS[0], EOS_R8_GAINS[1], EOS_R8_GAINS[2]])
            .expect("EOS R8 gains are valid");
    let rejected_scale = normalized_rgb[1] / EOS_R8_GAINS[1];
    let rejected_gains = EOS_R8_GAINS.map(|gain| gain * rejected_scale);
    let rejected =
        profiled_pattern_mosaic(SIZE, SIZE, WHITE, rejected_gains, EOS_R8_XYZ_TO_CAMERA, |x, y| {
            phase_sample(direct_samples, x, y)
        });

    let unit_pixel = gpu.render(&unit, FRAME).center();
    let direct_pixel = gpu.render(&direct, FRAME).center();
    let rejected_pixel = gpu.render(&rejected, FRAME).center();
    for channel in 0..3 {
        assert!(
            direct_pixel[channel].abs_diff(unit_pixel[channel]) <= REFERENCE_TOLERANCE,
            "direct WB channel {channel}: {} vs unit reference {}",
            direct_pixel[channel],
            unit_pixel[channel]
        );
    }
    let expected = cpu_reference_byte(f64::from(NEUTRAL));
    for (channel, &actual) in direct_pixel[..3].iter().enumerate() {
        assert!(
            actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
            "direct WB channel {channel}: {actual} vs CPU reference {expected}"
        );
    }
    let direct_mean = direct_pixel[..3]
        .iter()
        .map(|&value| u16::from(value))
        .sum::<u16>()
        / 3;
    let rejected_mean = rejected_pixel[..3]
        .iter()
        .map(|&value| u16::from(value))
        .sum::<u16>()
        / 3;
    assert!(
        direct_mean >= rejected_mean + 8,
        "readback must distinguish direct WB ({direct_pixel:?}) from rejected Rec.709 normalization \
         ({rejected_pixel:?})"
    );
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

// ---------------------------------------------------------------------------
// Hue preservation of the tone/gamut mapping (open color question #4).
//
// The pipeline's tone map must be hue-preserving: per-channel ACES rotates
// the hue of saturated colors because each channel compresses differently.
// CIELAB h° is exactly invariant under uniform linear-RGB scaling, so a
// hue-preserving mapper shows ~0° shift modulo f32 math and 8-bit
// quantization; a per-channel mapper shows large shifts on mixed hues.

/// Tolerance for CIELAB hue preservation, in degrees. The hue-preserving
/// scheme is exact in linear RGB; the residual comes from f32 shader
/// arithmetic, the hardware sRGB conversion, unorm8 quantization (±0.5 code),
/// and the CIELAB linear toe (low-luminance saturated colors drift ~1° in h°
/// even under exact uniform scaling). Measured residuals on M5 are ≤0.91°,
/// so 3° is a wide safety margin that still catches any per-channel
/// regression (which shifts mixed hues by 10–50° — see SUMMARY.md).
const HUE_TOLERANCE_DEGREES: f64 = 3.0;

/// Linear sRGB component of an 8-bit code (IEC 61966-2-1 inverse EOTF).
fn byte_to_linear(byte: u8) -> f64 {
    let encoded = f64::from(byte) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// CIELAB hue angle (degrees, D65) of a linear sRGB triplet.
fn cielab_hue_degrees(linear_rgb: [f64; 3]) -> f64 {
    let [r, g, b] = linear_rgb;
    let matrix = rrrah_core::SRGB_TO_XYZ_D65_F64;
    let xyz = [
        matrix[0][0] * r + matrix[0][1] * g + matrix[0][2] * b,
        matrix[1][0] * r + matrix[1][1] * g + matrix[1][2] * b,
        matrix[2][0] * r + matrix[2][1] * g + matrix[2][2] * b,
    ];
    let white = rrrah_core::XYZ_WHITE_D65;
    let f = |t: f64| {
        let delta = 6.0 / 29.0;
        if t > delta * delta * delta {
            t.cbrt()
        } else {
            t / (3.0 * delta * delta) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(xyz[0] / white[0]), f(xyz[1] / white[1]), f(xyz[2] / white[2]));
    let a = 500.0 * (fx - fy);
    let b_star = 200.0 * (fy - fz);
    b_star.atan2(a).to_degrees().rem_euclid(360.0)
}

/// Smallest absolute difference between two hue angles, in degrees.
fn hue_delta_degrees(first: f64, second: f64) -> f64 {
    let delta = (first - second).abs() % 360.0;
    delta.min(360.0 - delta)
}

/// Camera profile whose resulting `camera_to_rgb` is the identity: the exact
/// inverse of `SRGB_TO_XYZ_D65`, so the row-normalized product with the sRGB
/// primaries matrix is the unit matrix. Mosaic phases then carry linear sRGB
/// components directly, which is what the hue test targets.
fn srgb_native_profile() -> [[f32; 3]; 4] {
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).expect("sRGB primaries are invertible");
    [inverse[0], inverse[1], inverse[2], [0.0; 3]]
}

/// Uniform mosaic whose developed linear RGB equals `color * level` (sRGB
/// native camera profile, unit WB): red/green/blue CFA phases carry the
/// color's components, bilinear demosaic of a per-phase constant field is
/// exact.
fn saturated_mosaic(color: [f32; 3], level: f32) -> rrrah_core::DecodedMosaic {
    profiled_pattern_mosaic(256, 256, WHITE, [1.0; 4], srgb_native_profile(), |x, y| {
        let channel = [color[0], color[1], color[1], color[2]][((y & 1) * 2 + (x & 1)) as usize];
        (channel * level * WHITE).round() as u16
    })
}

/// Renders `color * level * 2^exposure_stops` and returns the center pixel.
fn render_saturated(
    gpu: &GpuReadback,
    color: [f32; 3],
    level: f32,
    exposure_stops: f32,
) -> [u8; 4] {
    let mosaic = saturated_mosaic(color, level);
    if exposure_stops == 0.0 {
        return gpu.render(&mosaic, FRAME).center();
    }
    let (crop_width, crop_height) = mosaic.metadata.display_dimensions();
    let fit = ((FRAME[0] - 32) as f32 / crop_width as f32).min((FRAME[1] - 32) as f32 / crop_height as f32);
    let cover = (FRAME[0] as f32 / crop_width as f32).max(FRAME[1] as f32 / crop_height as f32);
    let view = rrrah_gpu::ViewParameters {
        viewport: [FRAME[0] as f32, FRAME[1] as f32],
        zoom: cover / fit,
        exposure_stops,
        ..rrrah_gpu::ViewParameters::default()
    };
    gpu.render_with_view(&mosaic, view, FRAME).center()
}

#[test]
fn saturated_colors_preserve_hue_through_tone_map() {
    let Some(gpu) = gpu() else { return };
    // Primaries, secondaries, and mixed hues; per-channel tone mapping
    // rotates the mixed hues hardest (channels compress unequally).
    let colors: [(&str, [f32; 3]); 12] = [
        ("red", [1.0, 0.0, 0.0]),
        ("yellow", [1.0, 1.0, 0.0]),
        ("green", [0.0, 1.0, 0.0]),
        ("cyan", [0.0, 1.0, 1.0]),
        ("blue", [0.0, 0.0, 1.0]),
        ("magenta", [1.0, 0.0, 1.0]),
        ("orange", [1.0, 0.5, 0.0]),
        ("lime", [0.5, 1.0, 0.0]),
        ("azure", [0.0, 0.5, 1.0]),
        ("rose", [1.0, 0.0, 0.5]),
        ("spring", [0.0, 1.0, 0.5]),
        ("violet", [0.5, 0.0, 1.0]),
    ];
    // (level, exposure_stops): the effective pre-tone-map multiplier is
    // level * 2^stops, so the last two rows reach beyond the reference white.
    let levels: [(f32, f32); 5] = [(0.18, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (1.0, 2.0)];

    let mut rows = Vec::new();
    for (name, color) in colors {
        let input_hue = cielab_hue_degrees(color.map(f64::from));
        for (level, stops) in levels {
            let effective = f64::from(level) * 2.0_f64.powi(i32::from(stops as i32));
            let pixel = render_saturated(&gpu, color, level, stops);
            let output_linear = [
                byte_to_linear(pixel[0]),
                byte_to_linear(pixel[1]),
                byte_to_linear(pixel[2]),
            ];
            let output_hue = cielab_hue_degrees(output_linear);
            let delta = hue_delta_degrees(input_hue, output_hue);
            let expected = cpu_reference_rgb(color.map(|c| f64::from(c) * effective));
            rows.push((name, color, effective, input_hue, output_hue, delta, pixel, expected));
        }
    }
    eprintln!("\nhue preservation (CIELAB h°, input -> output, Δh):");
    let mut worst = 0.0_f64;
    let mut worst_case = String::new();
    for (name, _, effective, input_hue, output_hue, delta, pixel, _) in &rows {
        eprintln!(
            "  {name:8} x{effective:<4.2}  {input_hue:7.2} -> {output_hue:7.2}  Δh {delta:5.2}°  rgb {pixel:?}"
        );
        if *delta > worst {
            worst = *delta;
            worst_case = format!("{name} x{effective}");
        }
    }
    eprintln!("worst hue shift: {worst:.3}° ({worst_case})");
    for (name, _, effective, _, _, delta, pixel, expected) in rows {
        assert!(
            delta <= HUE_TOLERANCE_DEGREES,
            "{name} x{effective}: hue shifted {delta:.2}° (tolerance {HUE_TOLERANCE_DEGREES}°)"
        );
        // The GPU output must also match the hue-preserving CPU reference.
        for (channel, (&actual, expected)) in pixel.iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
                "{name} x{effective}: channel {channel} readback {actual} vs reference {expected}"
            );
        }
    }
}

#[test]
fn out_of_gamut_camera_colors_desaturate_matching_cpu_reference() {
    let Some(gpu) = gpu() else { return };
    let camera_to_rgb =
        rrrah_core::camera_to_linear_srgb(EOS_R8_XYZ_TO_CAMERA).expect("EOS R8 profile is valid");
    // Saturated single-phase inputs through the real EOS R8 profile produce
    // sub-zero linear components (out-of-gamut camera colors). The mapper
    // must desaturate them to the gamut boundary — the CPU reference below
    // encodes exactly that path.
    for (name, samples) in [
        ("pure red", [0.5_f32, 0.0, 0.0, 0.0]),
        ("pure green", [0.0, 0.5, 0.5, 0.0]),
        ("pure blue", [0.0, 0.0, 0.0, 0.5]),
    ] {
        let mosaic = profiled_pattern_mosaic(256, 256, WHITE, EOS_R8_GAINS, EOS_R8_XYZ_TO_CAMERA, |x, y| {
            (samples[((y & 1) * 2 + (x & 1)) as usize] * WHITE).round() as u16
        });
        let camera_rgb = [
            samples[0] * EOS_R8_GAINS[0],
            0.5 * (samples[1] * EOS_R8_GAINS[1] + samples[2] * EOS_R8_GAINS[3]),
            samples[3] * EOS_R8_GAINS[2],
        ];
        let linear_rgb = [
            f64::from(camera_to_rgb[0][0] * camera_rgb[0] + camera_to_rgb[0][1] * camera_rgb[1] + camera_to_rgb[0][2] * camera_rgb[2]),
            f64::from(camera_to_rgb[1][0] * camera_rgb[0] + camera_to_rgb[1][1] * camera_rgb[1] + camera_to_rgb[1][2] * camera_rgb[2]),
            f64::from(camera_to_rgb[2][0] * camera_rgb[0] + camera_to_rgb[2][1] * camera_rgb[1] + camera_to_rgb[2][2] * camera_rgb[2]),
        ];
        assert!(
            linear_rgb.iter().any(|value| *value < 0.0),
            "{name}: test must exercise the negative path, got {linear_rgb:?}"
        );
        let pixel = gpu.render(&mosaic, FRAME).center();
        let expected = cpu_reference_rgb(linear_rgb);
        eprintln!("out-of-gamut {name}: linear {linear_rgb:?} -> readback {pixel:?} vs reference {expected:?}");
        for (channel, (&actual, expected)) in pixel.iter().zip(expected).enumerate() {
            assert!(
                actual.abs_diff(expected) <= REFERENCE_TOLERANCE,
                "{name}: channel {channel} readback {actual} vs reference {expected}"
            );
        }
    }
}
