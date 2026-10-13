#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::{DecodeRequest, RlaAlphaMode, RlaColorSpace};
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
};
fn no_cancel() -> bool {
    false
}

#[test]
fn cached_rla_success_cannot_bypass_missing_alpha_or_color_interpretation() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster/rla-8-c3-a8-mixed1.rla");
    let mut request = DecodeRequest::new(path);
    request.rla_alpha_mode = Some(RlaAlphaMode::Straight);
    request.rla_color_space = Some(RlaColorSpace::Srgb);
    let budget = MemoryBudget::new(1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [30; 32],
        max_file_bytes: 1024 * 1024,
        max_pixels: 1024 * 1024,
        max_frames: 1,
        max_cache_entries: 10,
    };
    let mut cache = FingerprintCache::default();
    let valid = fingerprint_file(&request, policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(!valid.cache_hit);
    for missing_alpha in [true, false] {
        let mut invalid = request.clone();
        if missing_alpha {
            invalid.rla_alpha_mode = None;
        } else {
            invalid.rla_color_space = None;
        }
        let fresh_error = fingerprint_file(
            &invalid,
            policy,
            &budget,
            &mut FingerprintCache::default(),
            no_cancel,
        )
        .unwrap_err();
        let cached_error = fingerprint_file(&invalid, policy, &budget, &mut cache, no_cancel)
            .expect_err("cached success must not bypass a missing RLA interpretation");
        assert_eq!(format!("{cached_error:?}"), format!("{fresh_error:?}"));
        assert_eq!(budget.used(), 0);
    }
    let restored = fingerprint_file(&request, policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(restored.cache_hit);
    assert_eq!(valid.fingerprint, restored.fingerprint);
    assert_eq!(budget.used(), 0);
}
