#![cfg(feature = "decode")]

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

const GRAY_ALPHA: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 4, 0, 0, 0, 177,
    233, 220, 63, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 240, 191, 225, 255, 127, 0, 14, 122,
    4, 125, 168, 46, 150, 210, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const RGBA: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 6, 0, 0, 0, 27,
    224, 20, 180, 0, 0, 0, 18, 73, 68, 65, 84, 120, 156, 99, 248, 255, 255, 63, 3, 16, 55, 0, 241, 127, 0,
    66, 77, 10, 119, 105, 229, 140, 97, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const OPAQUE: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 2, 0, 0, 0, 148,
    130, 131, 227, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 248, 15, 3, 0, 44, 221, 8, 248, 222, 233, 19,
    137, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn png_gray_alpha_and_rgba_preserve_opacity_in_selected_frame_identity() {
    let dir = tempfile::tempdir().unwrap();
    let paths = ["gray-alpha.png", "rgba.png", "opaque.png"].map(|name| dir.path().join(name));
    for (path, encoded) in paths.iter().zip([GRAY_ALPHA, RGBA, OPAQUE]) {
        std::fs::write(path, encoded).unwrap();
    }
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    {
        let frames: Vec<_> = paths
            .iter()
            .map(|path| decode_selected_frame(&DecodeRequest::new(path), 100, &budget, || false).unwrap())
            .collect();
        for frame in &frames[..2] {
            let view = frame.view(|| false).unwrap();
            assert_eq!(view.dimensions(), (3, 1));
            for (x, alpha) in [0.0_f32, 128.0 / 255.0, 1.0].into_iter().enumerate() {
                let rgba = view.rgba(u32::try_from(x).unwrap(), 0).unwrap();
                assert!((rgba[3] - alpha).abs() < 1e-6, "pixel {x}: {rgba:?}");
                // Visible white must survive normalization, including partial opacity.
                if alpha > 0.0 {
                    for channel in &rgba[..3] {
                        assert!((*channel - 1.0).abs() < 1e-6, "pixel {x}: {rgba:?}");
                    }
                }
            }
        }
        assert!(frames[0].same_selected_frame(&frames[1], || false).unwrap());
        assert_eq!(
            frames[0].selected_frame_digest(|| false).unwrap(),
            frames[1].selected_frame_digest(|| false).unwrap()
        );
        for frame in &frames[..2] {
            assert!(!frame.same_selected_frame(&frames[2], || false).unwrap());
            assert_ne!(
                frame.selected_frame_digest(|| false).unwrap(),
                frames[2].selected_frame_digest(|| false).unwrap()
            );
        }
    }
    assert_eq!(budget.used(), 0);
}

const PALETTE_TRNS: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 2, 3, 0, 0, 0, 102,
    142, 252, 39, 0, 0, 0, 9, 80, 76, 84, 69, 255, 255, 255, 255, 255, 255, 255, 255, 255, 142, 74, 229, 254,
    0, 0, 0, 2, 116, 82, 78, 83, 0, 128, 155, 43, 78, 24, 0, 0, 0, 10, 73, 68, 65, 84, 120, 156, 99, 144, 0,
    0, 0, 26, 0, 25, 45, 136, 244, 54, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const GRAY_KEY: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 0, 0, 0, 0, 62,
    139, 75, 104, 0, 0, 0, 2, 116, 82, 78, 83, 0, 255, 91, 145, 34, 181, 0, 0, 0, 12, 73, 68, 65, 84, 120,
    156, 99, 248, 255, 159, 1, 0, 4, 255, 1, 255, 93, 125, 2, 135, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
    130,
];

const RGB_KEY: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 2, 0, 0, 0, 148,
    130, 131, 227, 0, 0, 0, 6, 116, 82, 78, 83, 0, 255, 0, 255, 0, 255, 55, 88, 27, 125, 0, 0, 0, 14, 73, 68,
    65, 84, 120, 156, 99, 248, 15, 6, 12, 12, 12, 0, 38, 227, 5, 251, 168, 60, 112, 216, 0, 0, 0, 0, 73, 69,
    78, 68, 174, 66, 96, 130,
];

const RGBA_KEY: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 1, 8, 6, 0, 0, 0, 27,
    224, 20, 180, 0, 0, 0, 17, 73, 68, 65, 84, 120, 156, 99, 248, 255, 255, 63, 3, 8, 3, 193, 127, 0, 54,
    214, 6, 250, 78, 131, 23, 38, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn png_trns_palette_and_color_keys_match_explicit_rgba() {
    let directory = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    for (group, variants) in [
        ("palette", vec![PALETTE_TRNS, RGBA]),
        ("key", vec![GRAY_KEY, RGB_KEY, RGBA_KEY]),
    ] {
        {
            let frames: Vec<_> = variants
                .iter()
                .enumerate()
                .map(|(i, bytes)| {
                    let path = directory.path().join(format!("{group}-{i}.png"));
                    std::fs::write(&path, bytes).unwrap();
                    decode_selected_frame(&DecodeRequest::new(path), 100, &budget, || false).unwrap()
                })
                .collect();
            for frame in &frames {
                let view = frame.view(|| false).unwrap();
                assert_eq!(view.dimensions(), (3, 1));
                let expected = if group == "palette" {
                    [0.0, 128.0 / 255.0, 1.0]
                } else {
                    [0.0, 0.0, 1.0]
                };
                for (x, alpha) in expected.into_iter().enumerate() {
                    let rgba = view.rgba(u32::try_from(x).unwrap(), 0).unwrap();
                    assert!((rgba[3] - alpha).abs() < 1e-6, "{group} pixel {x}: {rgba:?}");
                    if alpha > 0.0 {
                        let color = if group == "palette" { 1.0 } else { 0.0 };
                        for channel in &rgba[..3] {
                            assert!((*channel - color).abs() < 1e-6);
                        }
                    }
                }
                assert!(frames[0].same_selected_frame(frame, || false).unwrap());
                assert_eq!(
                    frames[0].selected_frame_digest(|| false).unwrap(),
                    frame.selected_frame_digest(|| false).unwrap()
                );
            }
        }
        assert_eq!(budget.used(), 0);
    }
}
