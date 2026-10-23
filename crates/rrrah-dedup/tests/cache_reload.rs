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

#[test]
fn persisted_cache_rechecks_content_and_preserves_previous_publication_on_cancel() {
    use rrrah_dedup::cache::CacheError;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.ppm");
    let disk = directory.path().join("fingerprints.bin");
    let red = b"P6\n1 1\n255\n\xff\0\0";
    let green = b"P6\n1 1\n255\n\0\xff\0";
    std::fs::write(&path, red).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let request = DecodeRequest::new(&path);
    let policy = FingerprintPolicy {
        recipe: [71; 32],
        max_file_bytes: 1024,
        max_pixels: 1,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut first = FingerprintCache::default();
    let original = fingerprint_file(&request, policy, &budget, &mut first, || false).unwrap();
    first.save_atomic(&disk, || false).unwrap();
    let published = std::fs::read(&disk).unwrap();
    let mut restored = FingerprintCache::load_file(&disk, 4, || false).unwrap();
    assert!(
        fingerprint_file(&request, policy, &budget, &mut restored, || false)
            .unwrap()
            .cache_hit
    );
    std::fs::write(&path, green).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), red.len() as u64);
    let changed = fingerprint_file(&request, policy, &budget, &mut restored, || false).unwrap();
    assert!(!changed.cache_hit);
    assert_ne!(changed.fingerprint, original.fingerprint);
    assert!(matches!(
        restored.save_atomic(&disk, || true),
        Err(CacheError::Cancelled)
    ));
    assert_eq!(std::fs::read(&disk).unwrap(), published);
    let mut after_cancel = FingerprintCache::load_file(&disk, 4, || false).unwrap();
    assert!(
        !fingerprint_file(&request, policy, &budget, &mut after_cancel, || false)
            .unwrap()
            .cache_hit
    );
    std::fs::write(&path, red).unwrap();
    let red_again = fingerprint_file(&request, policy, &budget, &mut after_cancel, || false).unwrap();
    assert!(red_again.cache_hit);
    assert_eq!(red_again.fingerprint, original.fingerprint);
    let mut corrupt = published;
    *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&disk, corrupt).unwrap();
    assert!(matches!(
        FingerprintCache::load_file(&disk, 4, || false),
        Err(CacheError::Invalid)
    ));
    let mut rebuilt = FingerprintCache::default();
    let fresh = fingerprint_file(&request, policy, &budget, &mut rebuilt, || false).unwrap();
    assert!(!fresh.cache_hit);
    assert_eq!(fresh.fingerprint, original.fingerprint);
    rebuilt.save_atomic(&disk, || false).unwrap();
    let mut repaired = FingerprintCache::load_file(&disk, 4, || false).unwrap();
    assert!(
        fingerprint_file(&request, policy, &budget, &mut repaired, || false)
            .unwrap()
            .cache_hit
    );
    assert_eq!(budget.used(), 0);
}
