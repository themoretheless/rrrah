#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    decode::decode_selected_frame_bounded,
    geometry::Transform,
    warp::{WarpPolicy, verify_pixels},
};
#[test]
fn native_jpeg_pixels_are_qualified_against_independent_libjpeg_readback() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    for id in [830, 898, 1084, 1294] {
        let admission = AnimationBudget {
            max_frames: 1,
            max_pixels: 100_000,
            max_file_bytes: 1024 * 1024,
        };
        let a = decode_selected_frame_bounded(
            &DecodeRequest::new(root.join(format!("photos/{id}-jpeg.jpg"))),
            admission,
            &budget,
            || false,
        )
        .unwrap();
        let b = decode_selected_frame_bounded(
            &DecodeRequest::new(root.join(format!("photos-jpeg-oracle/{id}.png"))),
            admission,
            &budget,
            || false,
        )
        .unwrap();
        let av = a.view(|| false).unwrap();
        let bv = b.view(|| false).unwrap();
        assert_eq!(av.dimensions(), bv.dimensions());
        let e = verify_pixels(
            &av,
            &bv,
            Transform {
                a: 1.,
                b: 0.,
                translation: [0.; 2],
            },
            WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 100_000,
            },
            || false,
        )
        .unwrap();
        assert_eq!(e.compared_pixels, e.source_pixels);
        assert_eq!(e.matched_pixels, e.compared_pixels);
        assert!(e.maximum_channel_error <= 0.03);
        drop(a);
        drop(b);
        assert_eq!(budget.used(), 0);
    }
}
