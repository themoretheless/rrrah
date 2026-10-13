#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::FingerprintPolicy,
    groups::{GroupBudget, GroupError},
    scan::{ScanError, VisualPolicy, confirm_pixel_groups, confirm_pixels, scan_visual},
    visual::VisualError,
};
use std::cell::Cell;

fn run(kind: u8, budget: &MemoryBudget, cancel: impl Fn() -> bool) -> Result<usize, ScanError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let files = [(1, DecodeRequest::new(&path)), (2, DecodeRequest::new(&path))];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 1,
        },
        max_files: 2,
        max_pairs: 1,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    match kind {
        0 => scan_visual(files, policy, budget, &mut FingerprintCache::default(), cancel).map(|report| {
            assert!(report.issues.is_empty());
            report.pairs.len()
        }),
        1 => confirm_pixels(files, [(1, 2)], policy, budget, cancel).map(|report| {
            assert!(report.issues.is_empty());
            report.equal.len()
        }),
        _ => confirm_pixel_groups(
            files,
            [(1, 2)],
            policy,
            GroupBudget {
                max_entries: 2,
                max_pairs: 1,
                max_pair_checks: 10,
            },
            budget,
            cancel,
        )
        .map(|report| {
            assert!(report.source_issues.is_empty());
            report.confirmation.equal.len()
        }),
    }
}

fn every_checkpoint(kind: u8) {
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let calls = Cell::new(0usize);
    assert_eq!(
        run(kind, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
        1
    );
    let checkpoints = calls.get();
    assert!(checkpoints > 2);
    assert_eq!(budget.used(), 0);
    for stop in 1..=checkpoints {
        calls.set(0);
        let result = run(kind, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(calls.get() >= stop, "kind {kind}, stop {stop} was not observed");
        assert!(
            matches!(
                result,
                Err(ScanError::Cancelled)
                    | Err(ScanError::Index(VisualError::Cancelled))
                    | Err(ScanError::Group(GroupError::Cancelled))
            ),
            "kind {kind}, stop {stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0, "kind {kind}, stop {stop}");
    }
    assert_eq!(run(kind, &budget, || false).unwrap(), 1);
    assert_eq!(budget.used(), 0);
    println!("scan-cancellation kind={kind} checkpoints={checkpoints}");
}

#[test]
fn visual_all_observed_one_shot_checkpoints_and_retry() {
    every_checkpoint(0);
}
#[test]
fn pixels_all_observed_one_shot_checkpoints_and_retry() {
    every_checkpoint(1);
}
#[test]
fn groups_all_observed_one_shot_checkpoints_and_retry() {
    every_checkpoint(2);
}
