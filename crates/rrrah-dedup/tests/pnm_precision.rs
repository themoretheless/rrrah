#![cfg(feature = "decode")]

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;

#[test]
fn pgm16_sample_order_and_low_bits_survive_selected_frame_normalization() {
    let dir = tempfile::tempdir().unwrap();
    let samples = [1_u16, 2, 256, 257, 0x1234, 0x1235, 65534, 65535];
    let binary = dir.path().join("binary.pgm");
    let ascii = dir.path().join("ascii.pgm");
    let swapped = dir.path().join("swapped.pgm");
    let changed = dir.path().join("changed.pgm");
    let mut bytes = b"P5\n8 1\n65535\n".to_vec();
    for value in samples {
        // PGM stores 16-bit samples most-significant byte first, independent of host order.
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    std::fs::write(&binary, &bytes).unwrap();
    let text = samples.map(|value| value.to_string()).join(" ");
    std::fs::write(
        &ascii,
        format!("P2\n# Same samples, different container bytes\n8 1\n65535\n{text}\n"),
    )
    .unwrap();
    let mut wrong_order = b"P5\n8 1\n65535\n".to_vec();
    for value in samples {
        wrong_order.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&swapped, wrong_order).unwrap();
    // Change only a low bit that would disappear in an 8-bit intermediate.
    *bytes.last_mut().unwrap() = 254;
    std::fs::write(&changed, bytes).unwrap();

    let budget = MemoryBudget::new(1024 * 1024);
    {
        let load = |path| decode_selected_frame(&DecodeRequest::new(path), 8, &budget, || false).unwrap();
        let original = load(&binary);
        let equivalent = load(&ascii);
        let wrong_order = load(&swapped);
        let low_bit_change = load(&changed);
        let view = original.view(|| false).unwrap();
        assert_eq!(view.dimensions(), (8, 1));
        for (x, sample) in samples.into_iter().enumerate() {
            let encoded = f64::from(sample) / 65535.0;
            // Independent full-range BT.709 inverse transfer reference.
            let reference = if encoded < 0.081 {
                encoded / 4.5
            } else {
                ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
            };
            let rgba = view.rgba(u32::try_from(x).unwrap(), 0).unwrap();
            for actual in &rgba[..3] {
                assert!((f64::from(*actual) - reference).abs() < 3e-7, "sample {sample}");
            }
            assert_eq!(rgba[3], 1.0);
        }
        assert!(original.same_selected_frame(&equivalent, || false).unwrap());
        assert_eq!(
            original.selected_frame_digest(|| false).unwrap(),
            equivalent.selected_frame_digest(|| false).unwrap()
        );
        for different in [&wrong_order, &low_bit_change] {
            assert!(!original.same_selected_frame(different, || false).unwrap());
            assert_ne!(
                original.selected_frame_digest(|| false).unwrap(),
                different.selected_frame_digest(|| false).unwrap()
            );
        }
    }
    assert_eq!(budget.used(), 0);
}
