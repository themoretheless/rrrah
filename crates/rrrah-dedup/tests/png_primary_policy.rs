#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, decode_selected_frame, fingerprint_file},
};
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31,
    21, 196, 137, 0, 0, 0, 4, 103, 65, 77, 65, 0, 0, 195, 80, 0, 153, 181, 52, 0, 0, 0, 11, 73, 68, 65, 84,
    120, 156, 99, 104, 0, 2, 0, 5, 5, 2, 1, 172, 83, 34, 58, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn explicit_png_primaries_preserve_gamma_alpha_and_cache_policy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gamma.png");
    std::fs::write(&path, PNG).unwrap();
    let strict = DecodeRequest::new(&path);
    let mut assumed = strict.clone();
    assumed.png_assumed_rgb_primaries = Some([0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290]);
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    assert!(decode_selected_frame(&strict, 1, &budget, || false).is_err());
    {
        let frame = decode_selected_frame(&assumed, 1, &budget, || false).unwrap();
        let view = frame.view(|| false).unwrap();
        let rgba = view.rgba(0, 0).unwrap();
        let reference = (128_f64 / 255.).powi(2);
        for value in &rgba[..3] {
            assert!((f64::from(*value) - reference).abs() < 1e-4, "{rgba:?}");
        }
        assert!((rgba[3] - 128_f32 / 255.).abs() < 1e-6);
    }
    assert_eq!(budget.used(), 0);
    let policy = FingerprintPolicy {
        recipe: [19; 32],
        max_file_bytes: 4096,
        max_pixels: 1,
        max_frames: 1,
        max_cache_entries: 8,
    };
    let mut cache = FingerprintCache::default();
    assert!(
        !fingerprint_file(&assumed, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        fingerprint_file(&assumed, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    let mut p3 = assumed.clone();
    p3.png_assumed_rgb_primaries = Some([0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290]);
    assert!(
        !fingerprint_file(&p3, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(fingerprint_file(&strict, policy, &budget, &mut cache, || false).is_err());
    let mut invalid = assumed.clone();
    invalid.png_assumed_rgb_primaries = Some([0.; 8]);
    assert!(fingerprint_file(&invalid, policy, &budget, &mut cache, || false).is_err());
    assert!(
        fingerprint_file(&assumed, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert_eq!(budget.used(), 0);
}

const APNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31,
    21, 196, 137, 0, 0, 0, 4, 103, 65, 77, 65, 0, 0, 195, 80, 0, 153, 181, 52, 0, 0, 0, 8, 97, 99, 84, 76, 0,
    0, 0, 2, 0, 0, 0, 0, 243, 141, 147, 112, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 0, 0, 90, 127, 48, 208, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156,
    99, 104, 0, 2, 0, 5, 5, 2, 1, 172, 83, 34, 58, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 10, 0, 0, 193, 12, 218, 4, 0, 0, 0, 17, 102, 100, 65, 84, 0, 0,
    0, 2, 120, 156, 99, 112, 112, 112, 104, 0, 0, 2, 197, 1, 65, 38, 161, 121, 246, 0, 0, 0, 0, 73, 69, 78,
    68, 174, 66, 96, 130,
];

#[test]
fn explicit_png_policy_applies_to_each_apng_selected_frame() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gamma.apng");
    std::fs::write(&path, APNG).unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let policy = FingerprintPolicy {
        recipe: [29; 32],
        max_file_bytes: 4096,
        max_pixels: 1,
        max_frames: 2,
        max_cache_entries: 8,
    };
    for (index, code) in [128_u8, 64].into_iter().enumerate() {
        let mut request = DecodeRequest::new(&path);
        request.image_index = index;
        assert!(decode_selected_frame(&request, 1, &budget, || false).is_err());
        request.png_assumed_rgb_primaries = Some([0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290]);
        {
            let frame = decode_selected_frame(&request, 1, &budget, || false).unwrap();
            let rgba = frame.view(|| false).unwrap().rgba(0, 0).unwrap();
            for value in &rgba[..3] {
                assert!(
                    (f64::from(*value) - (f64::from(code) / 255.).powi(2)).abs() < 1e-4,
                    "frame {index}: {rgba:?}"
                );
            }
            assert!((rgba[3] - 128_f32 / 255.).abs() < 1e-6);
        }
        assert_eq!(budget.used(), 0);
        assert!(
            !fingerprint_file(&request, policy, &budget, &mut cache, || false)
                .unwrap()
                .cache_hit
        );
        assert!(
            fingerprint_file(&request, policy, &budget, &mut cache, || false)
                .unwrap()
                .cache_hit
        );
        request.png_assumed_rgb_primaries = Some([0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290]);
        assert!(
            !fingerprint_file(&request, policy, &budget, &mut cache, || false)
                .unwrap()
                .cache_hit
        );
        assert_eq!(budget.used(), 0);
    }
}
