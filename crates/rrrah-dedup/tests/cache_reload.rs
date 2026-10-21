#![cfg(feature = "decode")]

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
};

#[test]
fn failed_reload_never_returns_warm_fingerprint_and_repair_retries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.ppm");
    let original = b"P6\n1 1\n255\n\xff\0\0";
    let changed = b"P6\n1 1\n255\n\0\xff\0";
    let request = DecodeRequest::new(&path);
    let policy = FingerprintPolicy {
        recipe: [7; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    std::fs::write(&path, original).unwrap();
    let first = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!first.cache_hit);
    assert_eq!(budget.used(), 0);
    assert!(
        fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    // Preserve the encoded length: a metadata/length shortcut must not reuse red pixels.
    for broken in [vec![0_u8; original.len()], b"P6\n1 1\n255\n".to_vec()] {
        std::fs::write(&path, broken).unwrap();
        for _ in 0..2 {
            assert!(fingerprint_file(&request, policy, &budget, &mut cache, || false).is_err());
            assert_eq!(budget.used(), 0);
        }
    }
    std::fs::remove_file(&path).unwrap();
    assert!(fingerprint_file(&request, policy, &budget, &mut cache, || false).is_err());
    assert_eq!(budget.used(), 0);
    std::fs::write(&path, changed).unwrap();
    let repaired = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(!repaired.cache_hit);
    assert_ne!(
        repaired.fingerprint.mean_linear_rgb,
        first.fingerprint.mean_linear_rgb
    );
    let warm = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(warm.cache_hit);
    assert_eq!(warm.fingerprint, repaired.fingerprint);
    std::fs::write(&path, original).unwrap();
    let restored = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
    assert!(restored.cache_hit);
    assert_eq!(restored.fingerprint, first.fingerprint);
    assert_eq!(budget.used(), 0);
}
