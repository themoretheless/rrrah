//! Independent EXIF corner ordering through RAW crop, CPU preview and GPU.
//! CIPA DC-008-2019, Figure 12: tag 6 is clockwise; tag 8 counterclockwise.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
mod common;
use common::{cpu_reference_rgb, profiled_pattern_mosaic};
use rrrah_core::{Orientation, Rect};

#[test]
fn all_exif_orientations_preserve_the_documented_corner_order() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("orientation adapter: {}", gpu.adapter_name());
    let inverse = rrrah_core::invert_3x3(rrrah_core::SRGB_TO_XYZ_D65).unwrap();
    let colors = [
        [0.4_f64, 0.05, 0.05],
        [0.05, 0.4, 0.05],
        [0.05, 0.05, 0.4],
        [0.4, 0.4, 0.05],
    ];
    let expected_colors = colors.map(cpu_reference_rgb);
    let mut mosaic = profiled_pattern_mosaic(
        64,
        48,
        60000.0,
        [1.0; 4],
        [inverse[0], inverse[1], inverse[2], [0.0; 3]],
        |x, y| {
            if !(12..52).contains(&x) || !(10..34).contains(&y) {
                return 0;
            }
            let quadrant = usize::from(y >= 22) * 2 + usize::from(x >= 32);
            let channel = [0, 1, 1, 2][((y % 2) * 2 + x % 2) as usize];
            (colors[quadrant][channel] * 60000.0).round() as u16
        },
    );
    mosaic.metadata.crop_area = Some(Rect::new(12, 10, 40, 24));
    // Fixed external corner permutations in TL,TR,BL,BR order. They do not
    // call production map_display_uv or reconstruct a transform from its enum.
    let cases = [
        (Orientation::Normal, [0, 1, 2, 3]),
        (Orientation::HorizontalFlip, [1, 0, 3, 2]),
        (Orientation::Rotate180, [3, 2, 1, 0]),
        (Orientation::VerticalFlip, [2, 3, 0, 1]),
        (Orientation::Transpose, [0, 2, 1, 3]),
        (Orientation::Rotate90, [2, 0, 3, 1]),
        (Orientation::Transverse, [3, 1, 2, 0]),
        (Orientation::Rotate270, [1, 3, 0, 2]),
    ];
    for (orientation, permutation) in cases {
        mosaic.metadata.orientation = orientation;
        let (width, height) = mosaic.metadata.thumbnail_dimensions(40);
        let thumbnail = mosaic.thumbnail_rgba8(40);
        let frame = gpu.render(&mosaic, [width, height]);
        let positions = [
            (width / 4, height / 4),
            (width * 3 / 4, height / 4),
            (width / 4, height * 3 / 4),
            (width * 3 / 4, height * 3 / 4),
        ];
        for (corner, (x, y)) in positions.into_iter().enumerate() {
            let expected = expected_colors[permutation[corner]];
            let actual = frame.pixel(x, y);
            let offset = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 2,
                    "GPU {orientation:?} corner {corner}: {actual:?} vs {expected:?}"
                );
                assert!(
                    thumbnail[offset + channel].abs_diff(expected[channel]) <= 2,
                    "thumbnail {orientation:?} corner {corner}"
                );
            }
        }
    }
}
