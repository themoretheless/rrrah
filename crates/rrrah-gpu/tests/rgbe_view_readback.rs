mod common;
use common::{cpu_reference_rgb, profiled_pattern_mosaic};
use rrrah_core::{CfaColor, CfaPattern, LevelGrid};
#[test]
fn four_plane_color_edges_and_downscale_match_cpu() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("RGBE view adapter: {}", gpu.adapter_name());
    let profile = [
        [0.7924, -0.1910, -0.0777],
        [-0.8226, 1.5459, 0.2998],
        [-0.1517, 0.2199, 0.6818],
        [-0.7242, 1.1401, 0.3481],
    ];
    let black = [492u16, 558, 491, 621];
    let gains = [1.55078125, 1., 2.84375, 1.];
    for dimensions in [[32, 24], [17, 13]] {
        for layout in [[3usize, 0, 2, 1], [0, 3, 1, 2], [1, 2, 0, 3]] {
            for levels in [[0.2, 0.3, 0.4, 0.5], [0.2, 0.3, 0.4, 0.9], [1.2, 0.7, 1.4, 1.1]] {
                let colors = [CfaColor::Red, CfaColor::Green, CfaColor::Blue, CfaColor::Emerald];
                let mut mosaic =
                    profiled_pattern_mosaic(dimensions[0], dimensions[1], 16368., gains, profile, |x, y| {
                        let phase = ((y % 2) * 2 + x % 2) as usize;
                        (f64::from(black[phase]) + levels[layout[phase]] * f64::from(16368 - black[phase]))
                            .round() as u16
                    });
                mosaic.metadata.cfa = Some(CfaPattern {
                    width: 2,
                    height: 2,
                    cells: layout.map(|c| colors[c]).to_vec(),
                });
                mosaic.metadata.black_level = LevelGrid {
                    width: 2,
                    height: 2,
                    components: 1,
                    values: black.map(f32::from).to_vec(),
                };
                let calibration = rrrah_core::rgbe::Calibration::new(&mosaic.metadata).unwrap();
                let expected = cpu_reference_rgb(
                    calibration
                        .reconstruct_linear_rgb([0, 0], |x, y| {
                            mosaic.pixels[(y * dimensions[0] + x) as usize]
                        })
                        .unwrap(),
                );
                let sizes = if dimensions == [32, 24] {
                    vec![dimensions, [8, 6]]
                } else {
                    vec![dimensions]
                };
                for size in sizes {
                    let frame = gpu.render(&mosaic, size);
                    let thumbnail = mosaic.thumbnail_rgba8(size[0]);
                    assert_eq!(thumbnail.len(), (size[0] * size[1] * 4) as usize);
                    for y in 0..size[1] {
                        for x in 0..size[0] {
                            let actual = frame.pixel(x, y);
                            let cpu = &thumbnail[((y * size[0] + x) * 4) as usize..][..4];
                            for c in 0..3 {
                                assert!(cpu[c].abs_diff(expected[c]) <= 2);
                            }
                            for c in 0..3 {
                                assert!(
                                    actual[c].abs_diff(expected[c]) <= 2,
                                    "layout {layout:?}, levels {levels:?}, pixel {x},{y}: {actual:?} vs {expected:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn odd_crop_orientations_and_tile_seams_preserve_rgbe_frame() {
    use rrrah_core::{Orientation, Rect};
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("RGBE tiling adapter: {}", gpu.adapter_name());
    let quad = [3usize, 0, 2, 1];
    let mut mosaic = profiled_pattern_mosaic(
        85,
        77,
        16368.,
        [1.55, 1., 2.84, 1.],
        [
            [0.7924, -0.1910, -0.0777],
            [-0.8226, 1.5459, 0.2998],
            [-0.1517, 0.2199, 0.6818],
            [-0.7242, 1.1401, 0.3481],
        ],
        |x, y| {
            let channel = quad[((y % 2) * 2 + x % 2) as usize];
            (700 + channel * 1300 + x as usize * 91 + y as usize * 53) as u16
        },
    );
    mosaic.metadata.cfa = Some(CfaPattern {
        width: 2,
        height: 2,
        cells: vec![CfaColor::Emerald, CfaColor::Red, CfaColor::Blue, CfaColor::Green],
    });
    mosaic.metadata.black_level = LevelGrid {
        width: 2,
        height: 2,
        components: 1,
        values: vec![492., 558., 491., 621.],
    };
    mosaic.metadata.crop_area = Some(Rect::new(3, 1, 75, 69));
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
        let (w, h) = mosaic.metadata.display_dimensions();
        for size in [[w, h], [w * 3, h * 3], [w.div_ceil(3), h.div_ceil(3)]] {
            let expected = gpu.render(&mosaic, size);
            for tile in [32, 33] {
                let actual = gpu.render_with_tiling(
                    &mosaic,
                    size,
                    rrrah_gpu::TilingOverrides {
                        tile_size: Some(tile),
                        tile_halo: Some(1),
                    },
                );
                assert_eq!(
                    actual.pixels, expected.pixels,
                    "orientation {orientation:?}, tile {tile}, size {size:?}"
                );
            }
        }
        let thumb = mosaic.thumbnail_rgba8(w.max(h));
        assert_eq!(thumb.len(), (w * h * 4) as usize);
        assert!(thumb.chunks_exact(4).all(|p| p[3] == 255));
    }
}
