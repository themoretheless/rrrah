#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::{AnimationBudget, AnimationKind},
    container::{ContainerError, Presentation},
    pages::PageKind,
    presentation_kind::detect_presentation,
};
#[test]
fn content_detection_ignores_extensions_and_preserves_sensor_routing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir().unwrap();
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let cases = [
        ("tests/fixtures/timeline/base.gif", 0),
        ("tests/fixtures/timeline/split.apng", 1),
        ("tests/fixtures/timeline/base.webp", 2),
        ("tests/fixtures/tiff/classic-little-none-same.tif", 3),
        ("../../tests/fixtures/raster/two-pages.dcx", 4),
        ("../../tests/fixtures/raster/pattern.ico.cur", 5),
        ("../../tests/fixtures/raster/pattern.png", 6),
    ];
    for (name, expected) in cases {
        let path = dir.path().join("renamed.bin");
        std::fs::copy(root.join(name), &path).unwrap();
        let mode = detect_presentation(&DecodeRequest::new(path), limits, 4096, &budget, || false).unwrap();
        let actual = match mode {
            Presentation::Animation(AnimationKind::Gif) => 0,
            Presentation::Animation(AnimationKind::Apng) => 1,
            Presentation::Animation(AnimationKind::Webp) => 2,
            Presentation::Pages(PageKind::Tiff) => 3,
            Presentation::Pages(PageKind::Dcx) => 4,
            Presentation::Pages(PageKind::Icon) => 5,
            Presentation::SelectedFrame => 6,
        };
        assert_eq!(actual, expected, "{name}");
    }
    let raw = dir.path().join("camera.jpg");
    std::fs::write(&raw, b"II*\0\x10\0\0\0CR\x02\0\0\0\0\0").unwrap();
    assert!(matches!(
        detect_presentation(&DecodeRequest::new(raw), limits, 4096, &budget, || false).unwrap(),
        Presentation::SelectedFrame
    ));
    let apng = DecodeRequest::new(root.join("tests/fixtures/timeline/split.apng"));
    assert!(matches!(
        detect_presentation(&apng, limits, 1, &budget, || false),
        Err(ContainerError::Detection)
    ));
    assert!(matches!(
        detect_presentation(&apng, limits, 4096, &budget, || true),
        Err(ContainerError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}
