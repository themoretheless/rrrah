//! Real-camera transforms against independent `LibRaw` references through GPU display.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[path = "common/camera_profiles.rs"]
mod camera_profiles;
mod common;

use common::{cpu_reference_rgb, profiled_pattern_mosaic};
use rrrah_core::{CfaColor, CfaPattern, LevelGrid};

#[test]
fn real_camera_profiles_match_independent_linear_transforms() {
    for profile in camera_profiles::PROFILES {
        let actual = rrrah_core::camera_to_linear_srgb(profile.xyz).expect("valid camera profile");
        for (row, actual_row) in actual.iter().enumerate() {
            for (column, actual_value) in actual_row.iter().enumerate() {
                assert!(
                    (f64::from(*actual_value) - profile.reference[row][column]).abs() < 1e-6,
                    "{} transform [{row},{column}]",
                    profile.name
                );
            }
        }
    }
}

#[test]
fn real_camera_colors_match_gpu_readback() {
    // This qualification must fail if no device is available; a skip isn't proof.
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("RAW color qualification adapter: {}", gpu.adapter_name());
    for profile in camera_profiles::PROFILES {
        for color in [
            [0.18_f64; 3],
            [0.35, 0.15, 0.1],
            [0.1, 0.3, 0.25],
            [0.03, 0.02, 0.06],
        ] {
            // Camera-neutral and colored fields; sample quantization is included
            // in the independent expected value rather than hidden by tolerance.
            let samples: [u16; 4] = std::array::from_fn(|phase| {
                let channel = profile.cfa[phase];
                (f64::from(profile.black[phase])
                    + color[channel] / f64::from(profile.wb[channel])
                        * f64::from(profile.white - profile.black[phase]))
                .round() as u16
            });
            let mut mosaic =
                profiled_pattern_mosaic(64, 64, profile.white, profile.wb, profile.xyz, |x, y| {
                    samples[((y % 2) * 2 + x % 2) as usize]
                });
            mosaic.metadata.cfa = Some(CfaPattern {
                width: 2,
                height: 2,
                cells: profile
                    .cfa
                    .map(|channel| match channel {
                        0 => CfaColor::Red,
                        1 => CfaColor::Green,
                        2 => CfaColor::Blue,
                        _ => unreachable!(),
                    })
                    .to_vec(),
            });
            mosaic.metadata.black_level = LevelGrid {
                width: 2,
                height: 2,
                components: 1,
                values: profile.black.to_vec(),
            };
            let mut camera = [0_f64; 3];
            let mut counts = [0_u32; 3];
            for (phase, sample) in samples.iter().enumerate() {
                let channel = profile.cfa[phase];
                camera[channel] += (f64::from(*sample) - f64::from(profile.black[phase]))
                    / f64::from(profile.white - profile.black[phase])
                    * f64::from(profile.wb[channel]);
                counts[channel] += 1;
            }
            for channel in 0..3 {
                camera[channel] /= f64::from(counts[channel]);
            }
            let linear = profile
                .reference
                .map(|row| row.iter().zip(camera).map(|(a, b)| a * b).sum());
            let expected = cpu_reference_rgb(linear);
            let actual = gpu.render(&mosaic, [64, 64]).center();
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 2,
                    "{} camera field {color:?}: GPU {actual:?}, independent transform {expected:?}",
                    profile.name
                );
            }
            assert_eq!(actual[3], 255);
            let thumbnail = mosaic.thumbnail_rgba8(64);
            assert_eq!(thumbnail.len(), 64 * 64 * 4);
            let center = (32 * 64 + 32) * 4;
            for channel in 0..3 {
                assert!(
                    thumbnail[center + channel].abs_diff(expected[channel]) <= 2,
                    "{} color thumbnail differs from independent transform",
                    profile.name
                );
            }
        }
    }
}

#[test]
fn distinct_green_wb_is_applied_before_demosaic() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let profile = &camera_profiles::PROFILES[4];
    for cfa in [[0, 1, 1, 2], [1, 0, 2, 1]] {
        let wb = [2.0_f32, 1.0, 1.5, 1.25];
        let first_green = usize::from(cfa[0] != 1);
        let samples: [u16; 4] = std::array::from_fn(|phase| {
            let gain = if cfa[phase] == 1 && phase != first_green {
                wb[3]
            } else {
                wb[cfa[phase]]
            };
            (0.2 * 60000.0 / gain).round() as u16
        });
        let mut mosaic = profiled_pattern_mosaic(64, 64, 60000.0, wb, profile.xyz, |x, y| {
            samples[((y % 2) * 2 + x % 2) as usize]
        });
        mosaic.metadata.cfa = Some(CfaPattern {
            width: 2,
            height: 2,
            cells: cfa
                .map(|channel| match channel {
                    0 => CfaColor::Red,
                    1 => CfaColor::Green,
                    2 => CfaColor::Blue,
                    _ => unreachable!(),
                })
                .to_vec(),
        });
        let expected = cpu_reference_rgb([0.2; 3]);
        let frame = gpu.render(&mosaic, [64, 64]);
        for y in 8..56 {
            for x in 8..56 {
                let actual = frame.pixel(x, y);
                for channel in 0..3 {
                    assert!(
                        actual[channel].abs_diff(expected[channel]) <= 2,
                        "G2 correction at {x},{y} with CFA {cfa:?}: {actual:?} vs {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn color_thumbnail_uses_crop_and_oriented_dimensions() {
    let profile = &camera_profiles::PROFILES[4];
    let color = [0.35_f64, 0.15, 0.1];
    let samples: [u16; 4] = std::array::from_fn(|phase| {
        let channel = profile.cfa[phase];
        (color[channel] / f64::from(profile.wb[channel]) * 60000.0).round() as u16
    });
    let mut mosaic = profiled_pattern_mosaic(64, 48, 60000.0, profile.wb, profile.xyz, |x, y| {
        if (5..21).contains(&x) && (7..15).contains(&y) {
            samples[((y % 2) * 2 + x % 2) as usize]
        } else {
            0
        }
    });
    mosaic.metadata.crop_area = Some(rrrah_core::Rect::new(5, 7, 16, 8));
    assert_eq!(mosaic.metadata.thumbnail_dimensions(8), (8, 4));
    let thumbnail = mosaic.thumbnail_rgba8(8);
    assert_eq!(thumbnail.len(), 8 * 4 * 4);
    let normal_center = &thumbnail[(2 * 8 + 4) * 4..(2 * 8 + 4) * 4 + 4];
    assert!(
        normal_center[0] > normal_center[2] + 20,
        "RAW thumbnail must be colored: {normal_center:?}"
    );
    mosaic.metadata.orientation = rrrah_core::Orientation::Rotate90;
    assert_eq!(mosaic.metadata.thumbnail_dimensions(8), (4, 8));
    let rotated = mosaic.thumbnail_rgba8(8);
    assert_eq!(&rotated[(4 * 4 + 2) * 4..(4 * 4 + 2) * 4 + 4], normal_center);
    mosaic.metadata.xyz_to_camera = [[0.0; 3]; 4];
    assert!(
        mosaic.thumbnail_rgba8(8).is_empty(),
        "invalid profile must not produce a gray placeholder"
    );
}

#[test]
fn sensor_clipped_near_white_is_neutral_but_chromatic_highlights_keep_color() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let profile = &camera_profiles::PROFILES[4];
    let wb = [2.0_f32, 1.0, 1.5, 1.0];
    for (samples, neutral) in [
        ([39000_u16, 60000, 60000, 51000], true),
        ([60000, 12000, 12000, 6000], false),
    ] {
        let mosaic = profiled_pattern_mosaic(64, 64, 60000.0, wb, profile.xyz, |x, y| {
            samples[((y % 2) * 2 + x % 2) as usize]
        });
        let expected = if neutral {
            cpu_reference_rgb([1.0; 3])
        } else {
            let camera = [2.0, 0.2, 0.15];
            cpu_reference_rgb(
                profile
                    .reference
                    .map(|row| row.iter().zip(camera).map(|(a, b)| a * b).sum()),
            )
        };
        let frame = gpu.render(&mosaic, [64, 64]);
        let thumbnail = mosaic.thumbnail_rgba8(64);
        for channel in 0..3 {
            assert!(
                frame.center()[channel].abs_diff(expected[channel]) <= 2,
                "clipped highlight {neutral}: {:?} vs {expected:?}",
                frame.center()
            );
            assert!(thumbnail[(32 * 64 + 32) * 4 + channel].abs_diff(expected[channel]) <= 2);
        }
    }
}

#[test]
fn minification_integrates_all_sensor_cells_instead_of_aliasing_the_stride() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    // Every 8x8 footprint has 8 dark and 8 bright complete Bayer cells. The
    // old isolated sample picked one level; the correct linear mean is 0.25.
    let mosaic = profiled_pattern_mosaic(
        256,
        256,
        60000.0,
        [1.0; 4],
        [inverse[0], inverse[1], inverse[2], [0.0; 3]],
        |x, y| {
            if ((x / 2) + (y / 2)) % 2 == 0 { 24000 } else { 6000 }
        },
    );
    let thumbnail = mosaic.thumbnail_rgba8(32);
    let expected = cpu_reference_rgb([0.25; 3]);
    for pixel in thumbnail.chunks_exact(4) {
        for channel in 0..3 {
            assert!(
                pixel[channel].abs_diff(expected[channel]) <= 1,
                "CPU area mean: {pixel:?} vs {expected:?}"
            );
        }
    }
    let frame = gpu.render(&mosaic, [32, 32]);
    for y in 2..30 {
        for x in 2..30 {
            let actual = frame.pixel(x, y);
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 2,
                    "GPU area mean at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }
}
