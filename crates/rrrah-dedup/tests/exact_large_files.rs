//! Large logical files exercise u64 admission and bounded cancellation, without
//! reading every sparse byte. A separate dense 66MiB case reads complete files.
//! This does not qualify complete multi-GiB equality.
use rrrah_dedup::exact::{
    ContentSnapshot, IssueKind, Options, SnapshotError, compare_files, discover, scan, scan_verified,
};
use std::{cell::Cell, fs::File};

#[test]
fn dense_large_files_distinguish_changes_outside_spaced_samples() {
    use std::io::{Seek, SeekFrom, Write};
    let directory = tempfile::tempdir().unwrap();
    let paths = ["original", "copy", "changed-middle"].map(|name| directory.path().join(name));
    let chunk: Vec<u8> = (0..262_144).map(|index| (index % 251) as u8).collect();
    let mut source = File::create(&paths[0]).unwrap();
    for _ in 0..264 {
        source.write_all(&chunk).unwrap();
    }
    drop(source);
    let length = 66 * 1024 * 1024;
    assert_eq!(std::fs::metadata(&paths[0]).unwrap().len(), length);
    for path in &paths[1..] {
        std::fs::copy(&paths[0], path).unwrap();
    }
    // Beyond the 1MiB prefix, before the first spaced interior 1MiB sample.
    let offset = 2 * 1024 * 1024 + 17;
    let original_byte = chunk[offset as usize % chunk.len()];
    let mut changed = std::fs::OpenOptions::new().write(true).open(&paths[2]).unwrap();
    changed.seek(SeekFrom::Start(offset)).unwrap();
    changed.write_all(&[original_byte ^ 1]).unwrap();
    drop(changed);
    let mut expected = paths[..2].to_vec();
    expected.sort();
    for reverse in [false, true] {
        let mut roots = paths.to_vec();
        if reverse {
            roots.reverse();
        }
        for verified in [false, true] {
            let report = if verified {
                scan_verified(&roots, &Options::default(), || false)
            } else {
                scan(&roots, &Options::default(), || false)
            };
            assert!(report.complete(), "{report:?}");
            assert!(report.issues.is_empty() && report.aliases.is_empty());
            assert_eq!(report.files_observed, 3);
            assert_eq!(report.groups.len(), 1);
            assert_eq!(report.groups[0].bytes, length);
            let mut actual = report.groups[0].paths.clone();
            actual.sort();
            assert_eq!(actual, expected);
        }
    }
}

fn sparse(path: &std::path::Path, bytes: u64) {
    let file = File::create(path).unwrap();
    #[cfg(windows)]
    {
        // Mark the empty owned fixture before extending its logical length.
        drop(file);
        let executable =
            std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("Windows SystemRoot"))
                .join("System32/fsutil.exe");
        let output = std::process::Command::new(executable)
            .args(["sparse", "setflag"])
            .arg(path)
            .output()
            .expect("mark owned fixture sparse");
        assert!(output.status.success(), "sparse marking failed: {output:?}");
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_len(bytes)
            .unwrap();
    }
    #[cfg(not(windows))]
    file.set_len(bytes).unwrap();
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
