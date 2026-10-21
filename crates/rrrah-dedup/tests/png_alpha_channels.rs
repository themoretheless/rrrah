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
