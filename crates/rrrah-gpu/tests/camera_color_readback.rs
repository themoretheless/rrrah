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

#[test]
fn hero9_dual_illuminant_colors_match_independent_adobe_matrix_on_gpu() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("HERO9 neutral matrix adapter: {}", gpu.adapter_name());
    let first = rrrah_core::DngColorMatrix {
        xyz_to_camera: [
            [1.8331, -0.8166, -0.2478],
            [0.1391, 0.8961, -0.0367],
            [0.0822, 0.0662, 0.2597],
        ],
        illuminant: Some(3),
    };
    let second = rrrah_core::DngColorMatrix {
        xyz_to_camera: [
            [1.0344, -0.421, -0.062],
            [-0.2315, 1.0625, 0.1948],
            [0.0093, 0.1058, 0.5541],
        ],
        illuminant: Some(23),
    };
    let neutral = [0.463768, 1.0, 0.565121];
    let resolved = rrrah_core::resolve_dng_neutral_matrix(first, second, neutral).unwrap();
    let xyz = [
        resolved[0].map(|v| v as f32),
        resolved[1].map(|v| v as f32),
        resolved[2].map(|v| v as f32),
        [0.0; 3],
    ];
    let wb = [1.0 / neutral[0] as f32, 1.0, 1.0 / neutral[2] as f32, 1.0];
    // Independent Adobe DNG SDK combined camera -> linear sRGB matrix.
    let oracle = [
        [3.111112788461, -0.142492408390, -0.531465203539],
        [-0.573327995961, 1.582402514522, -0.560077110020],
        [-0.073554291016, -0.516786056631, 2.744364805178],
    ];
    for camera in [
        [0.08, 0.15, 0.10],
        [0.15, 0.15, 0.08],
        [0.06, 0.10, 0.15],
        [neutral[0] * 0.18, 0.18, neutral[2] * 0.18],
    ] {
        let samples = camera.map(|v| (v * 16383.0_f64).round() as u16);
        let mosaic = profiled_pattern_mosaic(64, 64, 16383.0, wb, xyz, |x, y| {
            samples[match (x % 2, y % 2) {
                (0, 0) => 0,
                (1, 1) => 2,
                _ => 1,
            }]
        });
        let quantized = samples.map(|v| f64::from(v) / 16383.0);
        let expected = cpu_reference_rgb(oracle.map(|row| (0..3).map(|c| row[c] * quantized[c]).sum()));
        let actual = gpu.render(&mosaic, [64, 64]).center();
        for c in 0..3 {
            assert!(
                actual[c].abs_diff(expected[c]) <= 2,
                "camera {camera:?}: GPU {actual:?}, oracle {expected:?}"
            );
        }
        assert_eq!(actual[3], 255);
    }
}

#[test]
fn six_official_gpr_camera_fields_match_independent_color_oracles_on_metal() {
    let gpu = common::qualification_gpu().expect("required adapter");
    eprintln!("six GPR camera color adapter: {}", gpu.adapter_name());
    let cases = [
        (
            "HERO5",
            [
                [1.1683671474456787, -0.4118957817554474, -0.11441797763109207],
                [-0.1622902750968933, 1.0198734998703003, 0.11364110559225082],
                [0.016869492828845978, 0.10879307240247726, 0.37446898221969604],
                [0.0, 0.0, 0.0],
            ],
            [1.7238763570785522, 1.0, 1.8574206829071045, 1.0],
            [
                [2.699434753045, -0.359804749693, -0.383427021776],
                [-0.584027450781, 1.644362897531, -0.567563118746],
                [-0.07008416029, -0.539399896744, 2.935342622547],
            ],
        ),
        (
            "HERO6",
            [
                [1.0707875490188599, -0.39587464928627014, -0.09107314795255661],
                [-0.21422526240348816, 1.0516746044158936, 0.14000961184501648],
                [0.0075407992117106915, 0.11355993151664734, 0.41419801115989685],
                [0.0, 0.0, 0.0],
            ],
            [1.9140623807907104, 1.0, 1.749998688697815, 1.0],
            [
                [2.971880796467, -0.294707476807, -0.451409839939],
                [-0.57527209785, 1.630321043968, -0.577098312683],
                [-0.069661036504, -0.513189195494, 2.711769151294],
            ],
        ),
        (
            "HERO7",
            [
                [1.156180739402771, -0.4170306324958801, -0.113557368516922],
                [-0.17210394144058228, 1.0301159620285034, 0.11133851855993271],
                [0.014730697497725487, 0.11099880188703537, 0.36401277780532837],
                [0.0, 0.0, 0.0],
            ],
            [1.769532561302185, 1.0, 1.894531488418579, 1.0],
            [
                [2.724762203928, -0.327213699091, -0.402985143073],
                [-0.578825367693, 1.625224718768, -0.564789364987],
                [-0.07261241217, -0.549877065062, 3.014205111861],
            ],
        ),
        (
            "HERO9",
            [
                [1.044323205947876, -0.4304388761520386, -0.08494297415018082],
                [-0.2407032698392868, 1.086214303970337, 0.14262491464614868],
                [0.003242534352466464, 0.1176968440413475, 0.41469717025756836],
                [0.0, 0.0, 0.0],
            ],
            [2.156250476837158, 1.0, 1.769532561302185, 1.0],
            [
                [3.111112788461, -0.14249240839, -0.531465203539],
                [-0.573327995961, 1.582402514522, -0.56007711002],
                [-0.073554291016, -0.516786056631, 2.744364805178],
            ],
        ),
        (
            "Fusion-back",
            [
                [1.0539697408676147, -0.41059213876724243, -0.0804976299405098],
                [-0.22486136853694916, 1.061760425567627, 0.15823648869991302],
                [0.007290796842426062, 0.11183753609657288, 0.45956096053123474],
                [0.0, 0.0, 0.0],
            ],
            [2.0263671875, 1.0, 1.6479487419128418, 1.0],
            [
                [3.091137046769, -0.230343139553, -0.486333472648],
                [-0.584998231003, 1.636826060977, -0.57370523795],
                [-0.066927586934, -0.485604757587, 2.502629500216],
            ],
        ),
        (
            "Fusion-front",
            [
                [1.0539697408676147, -0.41059213876724243, -0.0804976299405098],
                [-0.22486136853694916, 1.061760425567627, 0.15823648869991302],
                [0.007290796842426062, 0.11183753609657288, 0.45956096053123474],
                [0.0, 0.0, 0.0],
            ],
            [2.0263671875, 1.0, 1.6479487419128418, 1.0],
            [
                [3.091137046769, -0.230343139553, -0.486333472648],
                [-0.584998231003, 1.636826060977, -0.57370523795],
                [-0.066927586934, -0.485604757587, 2.502629500216],
            ],
        ),
    ];
    for (name, xyz, wb, oracle) in cases {
        let neutral = [1.0 / f64::from(wb[0]), 1.0, 1.0 / f64::from(wb[2])];
        for camera in [
            [0.08, 0.15, 0.10],
            [0.15, 0.15, 0.08],
            [0.06, 0.10, 0.15],
            [neutral[0] * 0.18, 0.18, neutral[2] * 0.18],
        ] {
            let samples = camera.map(|v| (v * 16383.0_f64).round() as u16);
            let mosaic = profiled_pattern_mosaic(64, 64, 16383.0, wb, xyz, |x, y| {
                samples[match (x % 2, y % 2) {
                    (0, 0) => 0,
                    (1, 1) => 2,
                    _ => 1,
                }]
            });
            let quantized = samples.map(|v| f64::from(v) / 16383.0);
            let expected = cpu_reference_rgb(oracle.map(|row| (0..3).map(|c| row[c] * quantized[c]).sum()));
            let actual = gpu.render(&mosaic, [64, 64]).center();
            for c in 0..3 {
                assert!(
                    actual[c].abs_diff(expected[c]) <= 2,
                    "{name} camera {camera:?}: GPU {actual:?}, independent {expected:?}"
                );
            }
            assert_eq!(actual[3], 255);
        }
        eprintln!("{name}: four independent fields passed");
    }
}

#[test]
fn all_bayer_phases_reconstruct_affine_color_fields_on_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Affine demosaic qualification adapter: {}", gpu.adapter_name());
    let profile = &camera_profiles::PROFILES[4];
    let wb = [1.7_f32, 1.0, 1.3, 1.25];
    // Bilinear interpolation reproduces affine fields analytically. Distinct
    // gradients per channel reveal swapped CFA phases; unequal greens reveal
    // applying white balance after interpolation. No shader sampler is reused.
    let field = |x: u32, y: u32| {
        [
            0.10 + f64::from(x) * 0.002 + f64::from(y) * 0.001,
            0.12 + f64::from(x) * 0.001 + f64::from(y) * 0.002,
            0.15 + f64::from(x) * 0.0015 - f64::from(y) * 0.0005,
        ]
    };
    for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
        let first_green = usize::from(cfa[0] != 1);
        let mut mosaic = profiled_pattern_mosaic(64, 64, 60000.0, wb, profile.xyz, |x, y| {
            let phase = ((y % 2) * 2 + x % 2) as usize;
            let channel = cfa[phase];
            let gain = if channel == 1 && phase != first_green {
                wb[3]
            } else {
                wb[channel]
            };
            (field(x, y)[channel] * 60000.0 / f64::from(gain)).round() as u16
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
        let frame = gpu.render(&mosaic, [64, 64]);
        for y in 2..62 {
            for x in 2..62 {
                let camera = field(x, y);
                let expected = cpu_reference_rgb(profile.reference.map(|row| {
                    row.iter()
                        .zip(camera)
                        .map(|(coefficient, value)| coefficient * value)
                        .sum()
                }));
                let actual = frame.pixel(x, y);
                for channel in 0..3 {
                    assert!(
                        actual[channel].abs_diff(expected[channel]) <= 2,
                        "CFA {cfa:?} at {x},{y}: {actual:?} vs affine oracle {expected:?}"
                    );
                }
                assert_eq!(actual[3], 255);
            }
        }
    }
}

#[test]
fn bayer_sensor_boundaries_and_small_tiles_match_cpu_preview() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Bayer boundary qualification adapter: {}", gpu.adapter_name());
    let profile = &camera_profiles::PROFILES[4];
    for side in [63, 64] {
        for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
            let mut mosaic =
                profiled_pattern_mosaic(side, side, 60000.0, [1.7, 1.0, 1.3, 1.25], profile.xyz, |x, y| {
                    (2000 + (x * 137 + y * 719 + x * y * 31) % 18000) as u16
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
            let expected = mosaic.thumbnail_rgba8(side);
            let normal = gpu.render(&mosaic, [side, side]);
            let tiled = gpu.render_with_tiling(
                &mosaic,
                [side, side],
                rrrah_gpu::TilingOverrides {
                    tile_size: Some(32),
                    tile_halo: Some(1),
                },
            );
            for y in 0..side {
                for x in 0..side {
                    let actual = normal.pixel(x, y);
                    assert_eq!(
                        actual,
                        tiled.pixel(x, y),
                        "tile seam at {x},{y}, side {side}, CFA {cfa:?}"
                    );
                    let offset = ((y * side + x) * 4) as usize;
                    for channel in 0..3 {
                        assert!(
                            actual[channel].abs_diff(expected[offset + channel]) <= 2,
                            "boundary at {x},{y}, side {side}, CFA {cfa:?}: {actual:?} vs {:?}",
                            &expected[offset..offset + 4]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cropped_bayer_all_orientations_match_cpu_and_tiled_metal() {
    use rrrah_core::Orientation;
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Oriented Bayer qualification adapter: {}", gpu.adapter_name());
    let profile = &camera_profiles::PROFILES[4];
    for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
        let mut mosaic =
            profiled_pattern_mosaic(71, 57, 60000.0, [1.7, 1.0, 1.3, 1.25], profile.xyz, |x, y| {
                (2000 + (x * 137 + y * 719 + x * y * 31) % 18000) as u16
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
        mosaic.metadata.crop_area = Some(rrrah_core::Rect::new(3, 5, 65, 49));
        for orientation in [
            Orientation::Normal,
            Orientation::HorizontalFlip,
            Orientation::Rotate180,
            Orientation::VerticalFlip,
            Orientation::Transpose,
            Orientation::Rotate90,
            Orientation::Transverse,
            Orientation::Rotate270,
        ] {
            mosaic.metadata.orientation = orientation;
            let (width, height) = mosaic.metadata.display_dimensions();
            let expected = mosaic.thumbnail_rgba8(width.max(height));
            let normal = gpu.render(&mosaic, [width, height]);
            let tiled = gpu.render_with_tiling(
                &mosaic,
                [width, height],
                rrrah_gpu::TilingOverrides {
                    tile_size: Some(32),
                    tile_halo: Some(1),
                },
            );
            for y in 0..height {
                for x in 0..width {
                    let actual = normal.pixel(x, y);
                    assert_eq!(
                        actual,
                        tiled.pixel(x, y),
                        "tile seam {orientation:?} CFA {cfa:?} at {x},{y}"
                    );
                    let offset = ((y * width + x) * 4) as usize;
                    for channel in 0..3 {
                        assert!(
                            actual[channel].abs_diff(expected[offset + channel]) <= 2,
                            "{orientation:?} CFA {cfa:?} at {x},{y}: {actual:?} vs {:?}",
                            &expected[offset..offset + 4]
                        );
                    }
                    assert_eq!(actual[3], 255);
                }
            }
        }
    }
}

#[test]
fn minified_cropped_bayer_all_orientations_match_cpu_and_tiled_metal() {
    use rrrah_core::Orientation;
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Minified Bayer qualification adapter: {}", gpu.adapter_name());
    let profile = &camera_profiles::PROFILES[4];
    for cfa in [[0, 1, 1, 2], [2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
        let mut mosaic =
            profiled_pattern_mosaic(71, 57, 60000.0, [1.7, 1.0, 1.3, 1.25], profile.xyz, |x, y| {
                (2000 + (x * 137 + y * 719 + x * y * 31) % 18000) as u16
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
        mosaic.metadata.crop_area = Some(rrrah_core::Rect::new(3, 5, 64, 48));
        for orientation in [
            Orientation::Normal,
            Orientation::HorizontalFlip,
            Orientation::Rotate180,
            Orientation::VerticalFlip,
            Orientation::Transpose,
            Orientation::Rotate90,
            Orientation::Transverse,
            Orientation::Rotate270,
        ] {
            mosaic.metadata.orientation = orientation;
            let (width, height) = mosaic.metadata.display_dimensions();
            let (width, height) = (width / 4, height / 4);
            let expected = mosaic.thumbnail_rgba8(width.max(height));
            let normal = gpu.render(&mosaic, [width, height]);
            let tiled = gpu.render_with_tiling(
                &mosaic,
                [width, height],
                rrrah_gpu::TilingOverrides {
                    tile_size: Some(32),
                    tile_halo: Some(1),
                },
            );
            for y in 0..height {
                for x in 0..width {
                    let actual = normal.pixel(x, y);
                    assert_eq!(
                        actual,
                        tiled.pixel(x, y),
                        "tile seam {orientation:?} CFA {cfa:?} at {x},{y}"
                    );
                    let offset = ((y * width + x) * 4) as usize;
                    for channel in 0..3 {
                        assert!(
                            actual[channel].abs_diff(expected[offset + channel]) <= 2,
                            "{orientation:?} CFA {cfa:?} at {x},{y}: {actual:?} vs {:?}",
                            &expected[offset..offset + 4]
                        );
                    }
                    assert_eq!(actual[3], 255);
                }
            }
        }
    }
}
