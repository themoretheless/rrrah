//! Large logical files exercise u64 admission and bounded cancellation, without
//! reading every sparse byte. This does not qualify complete multi-GiB equality.
use rrrah_dedup::exact::{
    ContentSnapshot, IssueKind, Options, SnapshotError, compare_files, discover, scan, scan_verified,
};
use std::{cell::Cell, fs::File};

fn sparse(path: &std::path::Path, bytes: u64) {
    File::create(path).unwrap().set_len(bytes).unwrap();
    assert_eq!(std::fs::metadata(path).unwrap().len(), bytes);
}

#[test]
fn large_logical_lengths_cannot_wrap_byte_admission() {
    let directory = tempfile::tempdir().unwrap();
    let left = directory.path().join("large-left");
    let right = directory.path().join("large-right");
    for length in [u64::from(u32::MAX), u64::from(u32::MAX) + 1, (1_u64 << 40) + 17] {
        sparse(&left, length);
        sparse(&right, length);
        let calls = Cell::new(0);
        let checkpoint = || {
            calls.set(calls.get() + 1);
            assert!(calls.get() <= 3, "oversized source must refuse before hashing");
            false
        };
        assert!(matches!(
            ContentSnapshot::read(&left, length - 1, checkpoint),
            Err(SnapshotError::Policy)
        ));
        calls.set(0);
        assert!(matches!(
            compare_files(&left, &right, length - 1, checkpoint),
            Err(SnapshotError::Policy)
        ));
    }
}

#[test]
fn large_file_discovery_keeps_u64_lengths_and_reports_limit_omissions() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ["below", "above", "terabyte"].map(|name| directory.path().join(name));
    for (path, length) in paths
        .iter()
        .zip([u64::from(u32::MAX), u64::from(u32::MAX) + 1, (1_u64 << 40) + 17])
    {
        sparse(path, length);
    }
    let all = discover(&paths, &Options::default(), || false);
    assert!(!all.cancelled && all.issues.is_empty());
    assert_eq!(all.files.len(), 3);
    let limited = discover(
        &paths,
        &Options {
            max_file_bytes: u64::from(u32::MAX),
            ..Options::default()
        },
        || false,
    );
    assert!(!limited.cancelled);
    assert_eq!(limited.files, vec![paths[0].clone()]);
    assert_eq!(limited.issues.len(), 2);
    assert!(limited.issues.iter().all(|issue| issue.kind == IssueKind::Limit));
}

#[test]
fn large_sparse_reads_cancel_without_publishing_an_unread_group() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ["left", "right"].map(|name| directory.path().join(name));
    for path in &paths {
        sparse(path, u64::from(u32::MAX) + 65_537);
    }
    for verified in [false, true] {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= 32
        };
        let report = if verified {
            scan_verified(&paths, &Options::default(), cancel)
        } else {
            scan(&paths, &Options::default(), cancel)
        };
        assert!(report.cancelled && !report.complete());
        assert!(report.groups.is_empty());
        assert!(
            report.bytes_read > 0 && report.bytes_read <= 32 * 65_536,
            "{} bytes before cancellation",
            report.bytes_read
        );
    }
    let calls = Cell::new(0);
    let result = compare_files(&paths[0], &paths[1], u64::MAX, || {
        calls.set(calls.get() + 1);
        calls.get() >= 12
    });
    assert!(matches!(result, Err(SnapshotError::Cancelled)));
    assert_eq!(calls.get(), 12);
}
