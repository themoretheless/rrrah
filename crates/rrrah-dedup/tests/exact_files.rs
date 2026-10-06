use rrrah_dedup::exact::{IssueKind, Options, scan};
use std::{cell::Cell, fs};

#[test]
fn nested_overlapping_roots_match_the_full_authored_partition() {
    use std::collections::BTreeSet;
    let root = tempfile::tempdir().unwrap();
    let mut expected = BTreeSet::new();
    let mut roots = vec![root.path().to_path_buf()];
    for family in 0_u8..48 {
        let mut copies = Vec::new();
        for copy in 0..3 {
            let directory = root
                .path()
                .join(format!("branch-{copy}/level-{}/deep", family % 8));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join(format!("copy-{family:02}.data"));
            let mut bytes = vec![19; 16 * 1024];
            bytes[8192] = family;
            fs::write(&path, bytes).unwrap();
            copies.push(path);
            roots.push(directory);
        }
        copies.sort();
        expected.insert(copies.clone());
        // Same size and sampled edges, but distinct middle contents.
        let unique = root.path().join(format!("unique-{family:02}.data"));
        let mut bytes = vec![19; 16 * 1024];
        bytes[8192] = family;
        bytes[8193] = 21;
        fs::write(&unique, bytes).unwrap();
        roots.push(copies[0].clone());
    }
    roots.push(root.path().to_path_buf());
    let partition = |report: &rrrah_dedup::exact::Report| {
        report
            .groups
            .iter()
            .map(|group| {
                assert_eq!(group.bytes, 16 * 1024);
                let mut paths = group.paths.clone();
                paths.sort();
                paths
            })
            .collect::<BTreeSet<_>>()
    };
    let full = scan(&roots, &Options::default(), || false);
    assert!(full.complete(), "{:?}", full.issues);
    assert_eq!(full.files_observed, 192);
    assert_eq!(full.groups.len(), 48);
    assert_eq!(partition(&full), expected);
    roots.reverse();
    let reversed = scan(&roots, &Options::default(), || false);
    assert!(reversed.complete());
    assert_eq!(reversed.files_observed, 192);
    assert_eq!(partition(&reversed), expected);
    for options in [
        Options {
            max_entries: 20,
            ..Options::default()
        },
        Options {
            max_depth: 1,
            ..Options::default()
        },
    ] {
        let limited = scan(&[root.path().to_path_buf()], &options, || false);
        assert!(!limited.complete());
        assert!(limited.issues.iter().any(|issue| issue.kind == IssueKind::Limit));
        assert!(partition(&limited).is_subset(&expected));
    }
    let checks = Cell::new(0);
    let cancelled = scan(&roots, &Options::default(), || {
        checks.set(checks.get() + 1);
        checks.get() == 150
    });
    assert!(cancelled.cancelled);
    assert!(!cancelled.complete());
    let retry = scan(&roots, &Options::default(), || false);
    assert!(retry.complete());
    assert_eq!(partition(&retry), expected);
}

#[test]
fn copies_empty_files_and_equal_size_negatives() {
    let root = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("a", &b"alpha"[..]),
        ("b", &b"alpha"[..]),
        ("c", &b"omega"[..]),
        ("empty1", &b""[..]),
        ("empty2", &b""[..]),
    ] {
        fs::write(root.path().join(name), bytes).unwrap();
    }
    let report = scan(&[root.path().into()], &Options::default(), || false);
    assert!(report.complete(), "{:?}", report.issues);
    assert_eq!(report.groups.len(), 2);
    assert_eq!(
        report.groups[0].paths,
        vec![root.path().join("a"), root.path().join("b")]
    );
    assert_eq!(report.groups[0].digest, *blake3::hash(b"alpha").as_bytes());
    assert_eq!(report.groups[1].bytes, 0);
}

#[test]
fn same_samples_different_middle_are_not_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let bytes = vec![13; 128 * 1024];
    let mut different = bytes.clone();
    different[64 * 1024] ^= 1;
    fs::write(root.path().join("a"), &bytes).unwrap();
    fs::write(root.path().join("b"), different).unwrap();
    let report = scan(&[root.path().into()], &Options::default(), || false);
    assert!(report.complete());
    assert!(report.groups.is_empty());
    assert!(report.bytes_read >= 2 * bytes.len() as u64);
}

#[test]
fn missing_input_and_overlap_do_not_lose_valid_results() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    fs::create_dir(&child).unwrap();
    let a = child.join("a");
    fs::write(&a, b"same").unwrap();
    fs::write(child.join("b"), b"same").unwrap();
    let missing = root.path().join("missing");
    let report = scan(
        &[root.path().into(), child, a.clone(), a, missing.clone()],
        &Options::default(),
        || false,
    );
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.files_observed, 2);
    assert!(!report.complete());
    assert_eq!(report.issues[0].path, missing);
    assert!(matches!(report.issues[0].kind, IssueKind::Io(_)));
}

#[test]
fn size_depth_and_entry_limits_are_visible() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a"), [0; 100]).unwrap();
    for options in [
        Options {
            max_file_bytes: 99,
            ..Options::default()
        },
        Options {
            max_depth: 0,
            ..Options::default()
        },
        Options {
            max_entries: 0,
            ..Options::default()
        },
    ] {
        let report = scan(&[root.path().into()], &options, || false);
        assert!(!report.complete());
        assert!(report.issues.iter().any(|issue| issue.kind == IssueKind::Limit));
    }
}

#[test]
fn cancellation_during_full_hash_is_bounded() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a"), vec![1; 2 * 1024 * 1024]).unwrap();
    fs::write(root.path().join("b"), vec![1; 2 * 1024 * 1024]).unwrap();
    let checks = Cell::new(0);
    let report = scan(&[root.path().into()], &Options::default(), || {
        checks.set(checks.get() + 1);
        checks.get() >= 25
    });
    assert!(report.cancelled);
    assert!(!report.complete());
    assert!(report.groups.is_empty());
    assert!(report.bytes_read > 64 * 1024);
    assert!(report.bytes_read < 4 * 1024 * 1024);
}

#[test]
fn same_length_mutation_during_sample_is_detected() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, [1; 10000]).unwrap();
    fs::write(&b, [1; 10000]).unwrap();
    let checks = Cell::new(0);
    let report = scan(&[a.clone(), b], &Options::default(), || {
        checks.set(checks.get() + 1);
        if checks.get() == 4 {
            fs::write(&a, [2; 10000]).unwrap();
            // tmpfs may coalesce rapid writes to the same timestamp. Make this
            // metadata-invalidation fixture deterministic on coarse clocks.
            fs::File::options()
                .write(true)
                .open(&a)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))
                .unwrap();
        }
        false
    });
    assert!(report.groups.is_empty());
    assert!(!report.complete());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.path == a && issue.kind == IssueKind::Changed)
    );
}

#[cfg(unix)]
#[test]
fn hardlink_aliases_and_followed_symlink_cycles() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let hard = root.path().join("hard");
    let link = root.path().join("link");
    fs::write(&a, b"same").unwrap();
    fs::hard_link(&a, &hard).unwrap();
    symlink(&a, &link).unwrap();
    symlink(root.path(), root.path().join("cycle")).unwrap();
    let report = scan(&[root.path().into()], &Options::default(), || false);
    assert!(report.complete());
    assert_eq!(report.files_observed, 1);
    assert!(report.groups.is_empty());
    assert_eq!(report.aliases[0].paths, vec![a.clone(), hard.clone()]);
    let options = Options {
        follow_symlinks: true,
        ..Options::default()
    };
    let report = scan(&[root.path().into()], &options, || false);
    assert!(report.complete());
    assert_eq!(report.files_observed, 1);
    assert!(report.groups.is_empty());
    assert_eq!(report.aliases[0].paths, vec![a, hard, link]);
}

#[test]
fn changing_any_member_does_not_hide_healthy_duplicates() {
    // Sweep cancellation checkpoints to inject replacement during traversal,
    // hashing, comparison and final admission without timing-dependent threads.
    // A mutation after a member's last validation may remain unobserved: scans
    // are observations, not an atomic filesystem snapshot. Healthy copies must
    // remain grouped regardless of whether that late mutation was observed.
    let mut baseline_checks = 0;
    {
        let root = tempfile::tempdir().unwrap();
        let paths: Vec<_> = ["a", "b", "c", "d"].map(|name| root.path().join(name)).into();
        for path in &paths {
            fs::write(path, [1; 10000]).unwrap();
        }
        let calls = Cell::new(0);
        assert!(
            scan(&paths, &Options::default(), || {
                calls.set(calls.get() + 1);
                false
            })
            .complete()
        );
        baseline_checks += calls.get();
    }
    for member in 0..4 {
        for stop in 1..=baseline_checks {
            let root = tempfile::tempdir().unwrap();
            let paths: Vec<_> = ["a", "b", "c", "d"].map(|name| root.path().join(name)).into();
            for path in &paths {
                fs::write(path, [1; 10000]).unwrap();
            }
            let calls = Cell::new(0);
            let report = scan(&paths, &Options::default(), || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    fs::write(&paths[member], [2; 10000]).unwrap();
                }
                false
            });
            let healthy: Vec<_> = paths
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != member)
                .map(|(_, p)| p.clone())
                .collect();
            assert!(
                report
                    .groups
                    .iter()
                    .any(|g| healthy.iter().all(|p| g.paths.contains(p))),
                "member {member} stop {stop}: {report:?}"
            );
            assert!(
                report.issues.iter().all(|issue| issue.path == paths[member]),
                "wrong attribution: {report:?}"
            );
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_paths_preserve_exact_identity_and_duplicates() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join(OsString::from_vec(vec![b'a', 0xff]));
    let b = root.path().join(OsString::from_vec(vec![b'b', 0xfe]));
    fs::write(&a, b"same content").unwrap();
    fs::write(&b, b"same content").unwrap();
    let report = scan(&[root.path().into()], &Options::default(), || false);
    assert!(report.complete(), "{report:?}");
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].paths, vec![a, b]);
}

#[cfg(unix)]
#[test]
fn fifo_replacement_does_not_block_or_hide_healthy_copies() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<_> = ["a", "b", "c"].map(|name| root.path().join(name)).into();
    for path in &paths {
        fs::write(path, b"same content").unwrap();
    }
    let calls = Cell::new(0);
    let report = scan(&paths, &Options::default(), || {
        calls.set(calls.get() + 1);
        // Three traversal checks precede the first sample-hash check.
        if calls.get() == 4 {
            fs::remove_file(&paths[0]).unwrap();
            assert!(
                std::process::Command::new("mkfifo")
                    .arg(&paths[0])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        false
    });
    assert!(!report.complete());
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].paths, paths[1..]);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].path, paths[0]);
    assert_eq!(report.issues[0].kind, IssueKind::Changed);
    // A FIFO present before traversal is explicitly skipped as non-regular.
    let report = scan(&paths, &Options::default(), || false);
    assert_eq!(report.groups[0].paths, paths[1..]);
    assert_eq!(report.issues[0].kind, IssueKind::NotRegular);
}

#[cfg(unix)]
#[test]
fn unavailable_non_utf8_path_preserves_diagnostic_bytes() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let invalid = root.path().join(OsString::from_vec(vec![b'a', 0xff]));
    let report = scan(std::slice::from_ref(&invalid), &Options::default(), || false);
    assert!(!report.complete());
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].path, invalid);
    assert!(matches!(report.issues[0].kind, IssueKind::Io(_)));
}

#[cfg(unix)]
#[test]
fn unreadable_file_is_explicit_and_does_not_hide_healthy_copies() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let folder = tempfile::tempdir().unwrap();
    let paths = ["a", "b", "unreadable"].map(|name| folder.path().join(name));
    for path in &paths {
        fs::write(path, b"same content").unwrap();
    }
    let restore = Restore(paths[2].clone());
    fs::set_permissions(&paths[2], fs::Permissions::from_mode(0o000)).unwrap();
    // The prerequisite must actually deny reads, rather than merely setting mode bits.
    let denied = fs::read(&paths[2]);
    if denied.is_ok() {
        // Privileged runners can bypass Unix permission bits; their result cannot
        // qualify denial. The unprivileged fixture is exercised on the local host.
        drop(restore);
        return;
    }
    assert_eq!(denied.unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    let report = scan(&[folder.path().to_path_buf()], &Options::default(), || false);
    assert!(!report.complete());
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].paths, paths[..2]);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].path, paths[2]);
    assert!(matches!(report.issues[0].kind, IssueKind::Io(_)));
    drop(restore);
    let recovered = scan(&[folder.path().to_path_buf()], &Options::default(), || false);
    assert!(recovered.complete());
    assert_eq!(recovered.groups[0].paths, paths);
}

#[test]
fn verified_scan_preserves_groups_and_observes_every_admitted_file() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("a", &b"copy"[..]),
        ("b", &b"copy"[..]),
        ("c", &b"different length"[..]),
    ] {
        std::fs::write(dir.path().join(name), bytes).unwrap();
    }
    let roots = [dir.path().to_path_buf()];
    let ordinary = scan(&roots, &Options::default(), || false);
    let verified = rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || false);
    assert!(verified.complete(), "{verified:?}");
    assert_eq!(verified.groups, ordinary.groups);
    assert_eq!(verified.aliases, ordinary.aliases);
    assert_eq!(verified.files_observed, 3);
    assert!(verified.bytes_read >= ordinary.bytes_read + 2 * (4 + 4 + 16));
    let cancelled = rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || true);
    assert!(cancelled.cancelled && cancelled.groups.is_empty());
}

#[test]
fn verified_scan_discards_groups_for_one_shot_cancel_at_every_checkpoint() {
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in [
        ("a", &b"first"[..]),
        ("b", &b"first"[..]),
        ("c", &b"second-size"[..]),
        ("d", &b"second-size"[..]),
    ] {
        std::fs::write(dir.path().join(name), content).unwrap();
    }
    let roots = [dir.path().to_path_buf()];
    let checkpoints = Cell::new(0_usize);
    let baseline = rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || {
        checkpoints.set(checkpoints.get() + 1);
        false
    });
    assert!(baseline.complete());
    assert_eq!(baseline.groups.len(), 2);
    assert!(checkpoints.get() > 20);
    for cancelled_at in 1..=checkpoints.get() {
        let calls = Cell::new(0_usize);
        let report = rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || {
            calls.set(calls.get() + 1);
            calls.get() == cancelled_at
        });
        assert!(report.cancelled, "checkpoint {cancelled_at}: {report:?}");
        assert!(!report.complete());
        assert!(report.groups.is_empty(), "checkpoint {cancelled_at}: {report:?}");
    }
    let after = rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || false);
    assert!(after.complete());
    assert_eq!(after.groups, baseline.groups);
}

#[test]
fn physical_hardlinks_are_aliases_while_copied_bytes_are_separate_files() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let alias = root.path().join("alias");
    let copy = root.path().join("copy");
    fs::write(&original, b"physical identity fixture").unwrap();
    fs::hard_link(&original, &alias).unwrap();
    fs::copy(&original, &copy).unwrap();
    let report = scan(
        &[original.clone(), alias.clone(), copy.clone(), original.clone()],
        &Options::default(),
        || false,
    );
    assert!(report.complete(), "{:?}", report.issues);
    assert_eq!(report.files_observed, 2);
    assert_eq!(report.aliases.len(), 1);
    let mut expected_aliases = vec![original, alias];
    expected_aliases.sort();
    assert_eq!(report.aliases[0].paths, expected_aliases);
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].paths.len(), 2);
    assert!(report.groups[0].paths.contains(&copy));
}

#[test]
fn same_bytes_and_modified_time_do_not_hide_physical_file_replacement() {
    use rrrah_dedup::exact::{ContentSnapshot, SnapshotError};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    let retained = root.path().join("retained-original");
    let bytes = b"unchanged bytes, changed physical file";
    fs::write(&path, bytes).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let before = ContentSnapshot::read(&path, 1024, || false).unwrap();
    fs::rename(&path, &retained).unwrap();
    fs::write(&path, bytes).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read(&retained).unwrap(), fs::read(&path).unwrap());
    assert!(matches!(before.verify(|| false), Err(SnapshotError::Changed)));
    ContentSnapshot::read(&path, 1024, || false)
        .unwrap()
        .verify(|| false)
        .unwrap();
}
