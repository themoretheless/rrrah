#![cfg(feature = "decode")]

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

#[test]
fn single_axis_images_decode_fingerprint_and_confirm_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    for (width, height) in [(1, 1), (1, 2), (2, 1), (1, 17), (17, 1), (2, 2)] {
        let samples: Vec<u8> = (0..width * height)
            .flat_map(|index| {
                if index % 2 == 0 {
                    [0, 255, 0]
                } else {
                    [255, 0, 255]
                }
            })
            .collect();
        let binary = directory.path().join(format!("{width}-{height}.ppm"));
        let ascii = directory.path().join(format!("{width}-{height}-ascii.ppm"));
        let mut encoded = format!("P6\n{width} {height}\n255\n").into_bytes();
        encoded.extend_from_slice(&samples);
        std::fs::write(&binary, encoded).unwrap();
        let text = samples.iter().map(u8::to_string).collect::<Vec<_>>().join(" ");
        std::fs::write(&ascii, format!("P3\n{width} {height}\n255\n{text}\n")).unwrap();
        {
            let load =
                |path| decode_selected_frame(&DecodeRequest::new(path), 100, &budget, || false).unwrap();
            let first = load(&binary);
            let second = load(&ascii);
            let view = first.view(|| false).unwrap();
            assert_eq!(view.dimensions(), (width, height));
            for y in 0..height {
                for x in 0..width {
                    let expected = if (y * width + x) % 2 == 0 {
                        [0.0, 1.0, 0.0, 1.0]
                    } else {
                        [1.0, 0.0, 1.0, 1.0]
                    };
                    assert_eq!(view.rgba(x, y).unwrap(), expected);
                }
            }
            let fingerprint = view.fingerprint(|| false).unwrap();
            assert_eq!(fingerprint.compare(&fingerprint, true).distance, 0);
            assert!(first.same_selected_frame(&second, || false).unwrap());
            assert_eq!(
                first.selected_frame_digest(|| false).unwrap(),
                second.selected_frame_digest(|| false).unwrap()
            );
        }
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn zero_axis_headers_return_errors_without_retaining_budget() {
    let directory = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    for (width, height) in [(0, 0), (0, 2), (2, 0)] {
        for magic in ["P3", "P6"] {
            let path = directory.path().join(format!("{magic}-{width}-{height}.ppm"));
            std::fs::write(&path, format!("{magic}\n{width} {height}\n255\n")).unwrap();
            assert!(decode_selected_frame(&DecodeRequest::new(path), 100, &budget, || false).is_err());
            assert_eq!(budget.used(), 0);
        }
    }
}
