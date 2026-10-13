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
