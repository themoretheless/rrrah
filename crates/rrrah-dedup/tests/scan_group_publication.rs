#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    decode::FingerprintPolicy,
    groups::GroupBudget,
    scan::{ScanError, VisualPolicy, confirm_pixel_groups, confirm_pixels},
};
use std::{cell::Cell, fs};

#[test]
fn last_grouping_callback_mutation_cannot_publish_stale_pixel_group() {
    let root = tempfile::tempdir().unwrap();
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let a = root.path().join("a.png");
    let b = root.path().join("b.png");
    copy_writable(original, &a).unwrap();
    copy_writable(&a, &b).unwrap();
    let files = [(1, DecodeRequest::new(&a)), (2, DecodeRequest::new(&b))];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 0,
        },
        max_files: 2,
        max_pairs: 1,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let groups = GroupBudget {
        max_entries: 2,
        max_pairs: 1,
        max_pair_checks: 10,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let calls = Cell::new(0usize);
    let healthy = confirm_pixel_groups(files.clone(), [(1, 2)], policy, groups, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(healthy.grouping.groups, [vec![1, 2]]);
    assert_eq!(healthy.confirmation.equal, [(1, 2)]);
    let last = calls.get();
    assert!(last > 0);
    calls.set(0);
    let changed = Cell::new(false);
    let result = confirm_pixel_groups(files.clone(), [(1, 2)], policy, groups, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == last {
            let mut data = fs::read(&b).unwrap();
            data.push(0);
            fs::write(&b, data).unwrap();
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
        "stale group escaped final source observation: {result:?}"
    );
    assert_eq!(budget.used(), 0);
    copy_writable(&a, &b).unwrap();
    let retry = confirm_pixel_groups(files, [(1, 2)], policy, groups, &budget, || false).unwrap();
    assert_eq!(retry.grouping.groups, [vec![1, 2]]);
    assert!(retry.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn last_confirmation_callback_mutation_cannot_publish_stale_pixel_pair() {
    let root = tempfile::tempdir().unwrap();
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let a = root.path().join("a.png");
    let b = root.path().join("b.png");
    copy_writable(original, &a).unwrap();
    copy_writable(&a, &b).unwrap();
    let files = [(1, DecodeRequest::new(&a)), (2, DecodeRequest::new(&b))];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 0,
        },
        max_files: 2,
        max_pairs: 1,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let calls = Cell::new(0usize);
    let healthy = confirm_pixels(files.clone(), [(1, 2)], policy, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(healthy.equal, [(1, 2)]);
    let last = calls.get();
    assert!(last > 0);
    calls.set(0);
    let changed = Cell::new(false);
    let result = confirm_pixels(files.clone(), [(1, 2)], policy, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == last {
            let mut data = fs::read(&b).unwrap();
            data.push(0);
            fs::write(&b, data).unwrap();
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
        "stale group escaped final source observation: {result:?}"
    );
    assert_eq!(budget.used(), 0);
    copy_writable(&a, &b).unwrap();
    let retry = confirm_pixels(files, [(1, 2)], policy, &budget, || false).unwrap();
    assert_eq!(retry.equal, [(1, 2)]);
    assert!(retry.issues.is_empty());
    assert_eq!(budget.used(), 0);
}

// Fixture sources may be read-only in an immutable qualification checkout.
// Copy bytes without inheriting permissions so mutation tests can edit their own files.
fn copy_writable(source: impl AsRef<std::path::Path>, target: impl AsRef<std::path::Path>) -> std::io::Result<u64> {
    let bytes = std::fs::read(source)?;
    std::fs::write(target, &bytes)?;
    Ok(bytes.len() as u64)
}
