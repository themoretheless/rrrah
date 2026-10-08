#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;
use std::path::PathBuf;

// Author an EXIF IFD directly, preserving all JPEG scan and ICC bytes.
fn oriented_jpeg(source: &[u8], orientation: u16) -> Vec<u8> {
    assert_eq!(&source[..2], &[0xff, 0xd8]);
    let mut payload = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0".to_vec();
    payload.extend_from_slice(&[0x12, 0x01, 0x03, 0, 1, 0, 0, 0]);
    payload.extend_from_slice(&orientation.to_le_bytes());
    payload.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let mut result = vec![0xff, 0xd8, 0xff, 0xe1];
    result.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    result.extend_from_slice(&payload);
    let mut offset = 2;
    loop {
        let start = offset;
        assert_eq!(source[offset], 0xff);
        while source[offset] == 0xff {
            offset += 1;
        }
        let marker = source[offset];
        offset += 1;
        if marker == 0xda || marker == 0xd9 {
            result.extend_from_slice(&source[start..]);
            break;
        }
        assert!(!matches!(marker, 0x01 | 0xd0..=0xd8));
        let length = usize::from(u16::from_be_bytes([source[offset], source[offset + 1]]));
        assert!(length >= 2 && offset + length <= source.len());
        offset += length;
        if marker != 0xe1 {
            result.extend_from_slice(&source[start..offset]);
        }
    }
    result
}

#[test]
fn all_jpeg_exif_orientations_preserve_normalized_profiled_pixels() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let folder = tempfile::tempdir().unwrap();
    for name in ["cmyk-app14-profiled.jpg", "ycck-app14-profiled.jpg"] {
        let source = std::fs::read(root.join(name)).unwrap();
        let baseline_path = folder.path().join("baseline.jpg");
        std::fs::write(&baseline_path, oriented_jpeg(&source, 1)).unwrap();
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        let baseline =
            decode_selected_frame(&DecodeRequest::new(&baseline_path), 10000, &budget, || false).unwrap();
        let view = baseline.view(|| false).unwrap();
        let (width, height) = view.dimensions();
        assert_ne!(
            width, height,
            "Rectangular fixture must exercise swapped dimensions"
        );
        assert!(view.rgba(0, 0) != view.rgba(width - 1, height - 1));
        for orientation in 1..=8 {
            let path = folder.path().join(format!("{name}-{orientation}.jpg"));
            std::fs::write(&path, oriented_jpeg(&source, orientation)).unwrap();
            let decoded =
                decode_selected_frame(&DecodeRequest::new(&path), 10000, &budget, || false).unwrap();
            let actual = decoded.view(|| false).unwrap();
            let expected_size = if orientation < 5 {
                (width, height)
            } else {
                (height, width)
            };
            assert_eq!(actual.dimensions(), expected_size, "{name} EXIF{orientation}");
            for y in 0..expected_size.1 {
                for x in 0..expected_size.0 {
                    let (sx, sy) = match orientation {
                        1 => (x, y),
                        2 => (width - 1 - x, y),
                        3 => (width - 1 - x, height - 1 - y),
                        4 => (x, height - 1 - y),
                        5 => (y, x),
                        6 => (y, height - 1 - x),
                        7 => (width - 1 - y, height - 1 - x),
                        8 => (width - 1 - y, x),
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        actual.rgba(x, y),
                        view.rgba(sx, sy),
                        "{name} EXIF{orientation} at{x},{y}"
                    );
                }
            }
        }
        drop(baseline);
        assert_eq!(budget.used(), 0);
    }
}
