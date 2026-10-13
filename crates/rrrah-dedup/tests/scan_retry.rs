#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::FingerprintPolicy,
    scan::{VisualPolicy, confirm_pixels, scan_visual},
};

fn no_cancel() -> bool { false }

fn two_pixel_bmp() -> Vec<u8> {
    let mut bytes = vec![0; 62];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, 62_u32), (10, 54), (14, 40), (18, 2), (22, 1), (34, 8)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[57..60].fill(255);
    bytes
}

#[test]
fn same_cache_retries_repaired_sources_and_never_retains_old_pair_reports() {
    let directory = tempfile::tempdir().unwrap();
    let left = directory.path().join("left.bmp");
    let right = directory.path().join("right.bmp");
    let bytes = two_pixel_bmp();
    std::fs::write(&left, &bytes).unwrap();
    std::fs::write(&right, b"broken image").unwrap();
    let requests = vec![(2, DecodeRequest::new(&right)), (1, DecodeRequest::new(&left))];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [17; 32], max_file_bytes: 1024, max_pixels: 2,
            max_frames: 10, max_cache_entries: 10,
        },
        max_files: 2, max_pairs: 1, radius: 0,
        allow_transforms: false, require_information: true,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    let failed = scan_visual(requests.clone(), policy, &budget, &mut cache, no_cancel).unwrap();
    assert_eq!(failed.analysed, [1]);
    assert_eq!(failed.issues.len(), 1);
    assert_eq!(failed.issues[0].id, 2);
    assert!(failed.pairs.is_empty());

    std::fs::write(&right, &bytes).unwrap();
    let repaired = scan_visual(requests.clone(), policy, &budget, &mut cache, no_cancel).unwrap();
    assert_eq!(repaired.analysed, [1, 2]);
    assert!(repaired.issues.is_empty());
    assert_eq!(repaired.cache_hits, 2);
    assert_eq!(repaired.pairs.len(), 1);
    assert_eq!((repaired.pairs[0].left, repaired.pairs[0].right), (1, 2));

    let mut changed = bytes.clone();
    changed[54..57].fill(255);
    changed[57..60].fill(0);
    std::fs::write(&right, &changed).unwrap();
    let mutated = scan_visual(requests.clone(), policy, &budget, &mut cache, no_cancel).unwrap();
    let fresh = scan_visual(requests.clone(), policy, &budget, &mut FingerprintCache::default(), no_cancel).unwrap();
    assert_eq!(mutated.cache_hits, 1);
    assert_eq!(mutated.analysed, fresh.analysed);
    assert_eq!(mutated.pairs, fresh.pairs);
    assert!(mutated.issues.is_empty());
    let confirmed = confirm_pixels(requests.clone(), [(1, 2)], policy, &budget, no_cancel).unwrap();
    assert!(confirmed.equal.is_empty());
    assert_eq!(confirmed.different, [(1, 2)]);

    std::fs::remove_file(&right).unwrap();
    let removed = scan_visual(requests.clone(), policy, &budget, &mut cache, no_cancel).unwrap();
    assert_eq!(removed.analysed, [1]);
    assert_eq!(removed.issues.len(), 1);
    assert_eq!(removed.issues[0].id, 2);
    assert!(removed.pairs.is_empty());
    assert_eq!(repaired.pairs.len(), 1, "new scans must not mutate older reports");

    std::fs::write(&right, &bytes).unwrap();
    let retry = scan_visual(requests, policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(retry.issues.is_empty());
    assert_eq!(retry.cache_hits, 2);
    assert_eq!(retry.pairs, repaired.pairs);
    let empty = scan_visual([], policy, &budget, &mut cache, no_cancel).unwrap();
    assert!(empty.analysed.is_empty() && empty.pairs.is_empty() && empty.issues.is_empty());
    assert_eq!(budget.used(), 0);
}
