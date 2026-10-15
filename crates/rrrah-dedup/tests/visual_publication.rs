#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::FingerprintPolicy,
    scan::{ScanError, VisualPolicy, scan_visual},
};
use std::{cell::Cell, fs};

#[test]
fn flat_hash_collisions_remain_candidates_and_exact_pixel_groups_stay_separate() {
    let dir = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (index, shade) in [0_u8, 0, 255, 255].into_iter().enumerate() {
        let mut bmp = vec![0_u8; 58];
        bmp[..2].copy_from_slice(b"BM");
        for (offset, value) in [(2, 58_u32), (10, 54), (14, 40), (18, 1), (22, 1), (34, 4)] {
            bmp[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        bmp[54..57].fill(shade);
        let path = dir.path().join(format!("{index}.bmp"));
        fs::write(&path, bmp).unwrap();
        files.push((u64::try_from(index).unwrap() + 1, DecodeRequest::new(path)));
    }
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: 100_000,
            max_pixels: 30_000,
            max_frames: 1,
            max_cache_entries: 4,
        },
        max_files: 4,
        max_pairs: 6,
        radius: 0,
        allow_transforms: true,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    for warm in [false, true] {
        let candidates = scan_visual(files.clone(), policy, &budget, &mut cache, || false).unwrap();
        assert_eq!(candidates.analysed, [1, 2, 3, 4]);
        assert_eq!(candidates.pairs.len(), 6);
        assert!(candidates.pairs.iter().all(|pair| !pair.evidence.informative));
        assert!(candidates.issues.is_empty());
        // The first pass already reuses each identical-content copy within the scan.
        assert_eq!(candidates.cache_hits, if warm { 4 } else { 2 });
        let informative = scan_visual(
            files.clone(),
            VisualPolicy {
                require_information: true,
                ..policy
            },
            &budget,
            &mut cache,
            || false,
        )
        .unwrap();
        assert!(informative.pairs.is_empty());
        assert_eq!(informative.analysed, [1, 2, 3, 4]);
        assert_eq!(budget.used(), 0);
    }
    let exact = rrrah_dedup::scan::scan_equal_pixels(
        files,
        rrrah_dedup::scan::PixelSearchPolicy {
            decode: policy.fingerprint,
            max_files: 4,
            max_pairs: 6,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(exact.pixels.equal, [(1, 2), (3, 4)]);
    assert_eq!(exact.pixels.different.len(), 4);
    assert!(exact.pixels.issues.is_empty() && exact.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn final_visual_callback_mutation_refuses_cold_and_warm_reports() {
    for warm in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        let original =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information/gradient.png");
        fs::write(&a, fs::read(original).unwrap()).unwrap();
        fs::write(&b, fs::read(&a).unwrap()).unwrap();
        let files = [(1, DecodeRequest::new(&a)), (2, DecodeRequest::new(&b))];
        let policy = VisualPolicy {
            fingerprint: FingerprintPolicy {
                recipe: [0; 32],
                max_file_bytes: 100_000,
                max_pixels: 30_000,
                max_frames: 10,
                max_cache_entries: 2,
            },
            max_files: 2,
            max_pairs: 1,
            radius: 0,
            allow_transforms: false,
            require_information: false,
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut cache = FingerprintCache::default();
        if warm {
            scan_visual(files.clone(), policy, &budget, &mut cache, || false).unwrap();
        }
        let calls = Cell::new(0usize);
        let healthy = scan_visual(files.clone(), policy, &budget, &mut cache, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert!(healthy.issues.is_empty(), "{healthy:?}");
        assert_eq!(healthy.analysed, [1, 2]);
        assert_eq!(healthy.pairs.len(), 1);
        assert_eq!(healthy.cache_hits, if warm { 2 } else { 1 });
        if !warm {
            cache = FingerprintCache::default();
        }
        let last = calls.get();
        calls.set(0);
        let changed = Cell::new(false);
        let result = scan_visual(files.clone(), policy, &budget, &mut cache, || {
            calls.set(calls.get() + 1);
            if calls.get() == last {
                let mut bytes = fs::read(&b).unwrap();
                bytes.push(0);
                fs::write(&b, bytes).unwrap();
                changed.set(true);
            }
            false
        });
        assert!(changed.get());
        assert!(
            matches!(
                result,
                Err(ScanError::Source {
                    id: 2,
                    error: rrrah_dedup::exact::SnapshotError::Changed
                })
            ),
            "warm={warm}: stale visual report {result:?}"
        );
        assert_eq!(budget.used(), 0);
        fs::write(&b, fs::read(&a).unwrap()).unwrap();
        let retry = scan_visual(files, policy, &budget, &mut cache, || false).unwrap();
        assert_eq!(retry.analysed, [1, 2]);
        assert_eq!(retry.pairs.len(), 1);
        assert!(retry.issues.is_empty());
        assert_eq!(budget.used(), 0);
    }
}
