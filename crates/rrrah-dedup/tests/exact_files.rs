use rrrah_dedup::exact::{IssueKind, Options, scan};
use std::{cell::Cell, fs};

#[test]
fn exact_content_partitions_ignore_extensions_and_check_unsampled_bytes() {
    use std::collections::BTreeSet;
    let root = tempfile::tempdir().unwrap();
    let mut expected = BTreeSet::new();
    for (family, size) in [0, 512, 128 * 1024].into_iter().enumerate() {
        let bytes = vec![37_u8; size];
        let mut copies = Vec::new();
        for suffix in ["jpg", "PNG", "bin", ""] {
            let path = root.path().join(format!("family-{family}.{suffix}"));
            fs::write(&path, &bytes).unwrap();
            copies.push(path);
        }
        copies.sort();
        expected.insert(copies);
        if size > 0 {
            let mut negative = bytes;
            negative[size / 2] ^= 1;
            fs::write(root.path().join(format!("different-{family}.jpg")), negative).unwrap();
        }
    }
    let mut roots = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    roots.sort();
    for reverse in [false, true] {
        if reverse {
            roots.reverse();
        }
        for verified in [false, true] {
            let report = if verified {
                rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || false)
            } else {
                scan(&roots, &Options::default(), || false)
            };
            assert!(report.complete(), "{report:?}");
            assert_eq!(report.files_observed, 14);
            let actual = report
                .groups
                .iter()
                .map(|group| {
                    let mut paths = group.paths.clone();
                    paths.sort();
                    paths
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn reordered_middle_blocks_with_identical_sampled_edges_are_not_copies() {
    let root = tempfile::tempdir().unwrap();
    let paths = ["left.bin", "copy.bin", "permuted.bin"].map(|name| root.path().join(name));
    let prefix = vec![19_u8; 4 * 4096];
    let a = vec![37_u8; 4096];
    let b = vec![83_u8; 4096];
    let left = [prefix.as_slice(), &a, &b, &prefix].concat();
    let permuted = [prefix.as_slice(), &b, &a, &prefix].concat();
    assert_eq!(left.len(), permuted.len());
    assert_eq!(&left[..4096], &permuted[..4096]);
    assert_eq!(&left[left.len() - 4096..], &permuted[permuted.len() - 4096..]);
    assert_ne!(left, permuted);
    assert_ne!(blake3::hash(&left), blake3::hash(&permuted));
    fs::write(&paths[0], &left).unwrap();
    fs::write(&paths[1], &left).unwrap();
    fs::write(&paths[2], &permuted).unwrap();
    for roots in [paths.to_vec(), paths.iter().rev().cloned().collect()] {
        let report = scan(&roots, &Options::default(), || false);
        assert!(report.complete(), "{:?}", report.issues);
        assert_eq!(report.files_observed, 3);
        assert_eq!(report.groups.len(), 1);
        let mut actual = report.groups[0].paths.clone();
        actual.sort();
        let mut expected = paths[..2].to_vec();
        expected.sort();
        assert_eq!(actual, expected);
        assert_eq!(report.groups[0].bytes, left.len() as u64);
    }
}

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
        // Two root-admission callbacks precede the original sample checkpoint.
        if checks.get() == 6 {
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
fn lossy_equal_non_utf8_names_preserve_distinct_copy_partitions() {
    use std::{collections::BTreeSet, ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join(OsString::from_vec(vec![b'x', 0xff]));
    let b = root.path().join(OsString::from_vec(vec![b'x', 0xfe]));
    assert_ne!(a, b);
    assert_eq!(a.to_string_lossy(), b.to_string_lossy());
    let acopy = root.path().join("a-copy");
    let bcopy = root.path().join("b-copy");
    for path in [&a, &acopy] {
        fs::write(path, b"first family").unwrap();
    }
    for path in [&b, &bcopy] {
        fs::write(path, b"other family").unwrap();
    }
    let partition = |pairs: Vec<Vec<std::path::PathBuf>>| {
        pairs
            .into_iter()
            .map(|mut pair| {
                pair.sort();
                pair
            })
            .collect::<BTreeSet<_>>()
    };
    let expected = partition(vec![vec![a.clone(), acopy], vec![b.clone(), bcopy]]);
    for roots in [
        vec![root.path().to_path_buf(), a.clone(), b.clone()],
        vec![b, a, root.path().to_path_buf()],
    ] {
        let report = scan(&roots, &Options::default(), || false);
        assert!(report.complete(), "{report:?}");
        assert_eq!(report.files_observed, 4);
        assert_eq!(
            partition(report.groups.into_iter().map(|g| g.paths).collect()),
            expected
        );
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
        // Three root-admission callbacks precede the original pre-hash checkpoint.
        if calls.get() == 7 {
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

#[test]
fn repeated_exact_scan_rejects_stale_equality_after_rewrite_and_growth() {
    for growth in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::write(&a, b"same").unwrap();
        fs::write(&b, b"same").unwrap();
        let before = scan(&[a.clone(), b.clone()], &Options::default(), || false);
        assert!(before.complete());
        assert_eq!(before.groups.len(), 1);
        if growth {
            fs::write(&a, b"sameX").unwrap();
            fs::write(&b, b"sameY").unwrap();
        } else {
            fs::write(&b, b"edit").unwrap();
        }
        assert_ne!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        let after = scan(&[a, b], &Options::default(), || false);
        assert!(after.complete());
        assert!(after.groups.is_empty(), "growth={growth}: {after:?}");
    }
}

#[test]
fn restored_modified_time_does_not_hide_same_length_snapshot_edit() {
    use rrrah_dedup::exact::{ContentSnapshot, SnapshotError};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    fs::write(&path, vec![1; 128 * 1024]).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let before = ContentSnapshot::read(&path, 256 * 1024, || false).unwrap();
    fs::write(&path, vec![2; 128 * 1024]).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert!(matches!(before.verify(|| false), Err(SnapshotError::Changed)));
    let restored = ContentSnapshot::read(&path, 256 * 1024, || false).unwrap();
    restored.verify(|| false).unwrap();
}

#[cfg(unix)]
#[test]
fn restored_modified_time_does_not_hide_same_inode_edit_during_scan() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, [1; 10000]).unwrap();
    fs::write(&b, [1; 10000]).unwrap();
    let metadata = fs::metadata(&a).unwrap();
    let modified = metadata.modified().unwrap();
    let changed = (metadata.ctime(), metadata.ctime_nsec());
    // Advance to a different ctime tick without relying on subsecond clocks.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let checks = Cell::new(0);
    let report = scan(&[a.clone(), b.clone()], &Options::default(), || {
        checks.set(checks.get() + 1);
        // Two root-admission callbacks precede the original sample checkpoint.
        if checks.get() == 6 {
            fs::write(&a, [2; 10000]).unwrap();
            fs::File::options()
                .write(true)
                .open(&a)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            let after = fs::metadata(&a).unwrap();
            assert_eq!(
                (after.dev(), after.ino(), after.len()),
                (metadata.dev(), metadata.ino(), metadata.len())
            );
            assert_eq!(after.modified().unwrap(), modified);
            assert_ne!((after.ctime(), after.ctime_nsec()), changed);
        }
        false
    });
    assert!(checks.get() >= 4);
    assert!(report.groups.is_empty());
    assert!(!report.complete());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.path == a && issue.kind == IssueKind::Changed)
    );
    let fresh = scan(&[a, b], &Options::default(), || false);
    assert!(fresh.complete(), "{fresh:?}");
    assert!(fresh.groups.is_empty());
}

#[test]
fn excess_root_admission_refuses_before_missing_source_io() {
    let root = tempfile::tempdir().unwrap();
    let paths = vec![root.path().join("missing-a"), root.path().join("missing-b")];
    for roots in [paths.clone(), paths.iter().cloned().rev().collect()] {
        let report = scan(
            &roots,
            &Options {
                max_entries: 1,
                ..Options::default()
            },
            || false,
        );
        assert!(!report.complete());
        assert!(!report.cancelled);
        assert_eq!(report.files_observed, 0);
        assert!(report.groups.is_empty());
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].kind, IssueKind::Limit);
    }
    let zero = scan(
        &paths,
        &Options {
            max_entries: 0,
            ..Options::default()
        },
        || false,
    );
    assert_eq!(zero.issues.len(), 1);
    assert_eq!(zero.issues[0].kind, IssueKind::Limit);
    assert_eq!(zero.files_observed, 0);
}

#[test]
fn root_admission_cancels_before_io_at_each_preparation_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let paths = (0..64)
        .map(|n| root.path().join(format!("missing-{n}")))
        .collect::<Vec<_>>();
    for stop in 1..=paths.len() {
        let calls = Cell::new(0);
        let report = scan(&paths, &Options::default(), || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(report.cancelled, "stop={stop}");
        assert!(!report.complete());
        assert!(report.issues.is_empty(), "stop={stop}: {report:?}");
        assert!(report.groups.is_empty());
        assert_eq!(report.files_observed, 0);
        assert_eq!(report.bytes_read, 0);
        assert_eq!(calls.get(), stop);
    }
}

#[test]
fn duplicate_root_admission_retains_boundary_and_fresh_retry() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, b"same").unwrap();
    fs::write(&b, b"same").unwrap();
    let one = scan(
        &vec![a.clone(); 1000],
        &Options {
            max_entries: 1,
            ..Options::default()
        },
        || false,
    );
    assert!(one.complete(), "{one:?}");
    assert_eq!(one.files_observed, 1);
    let refused = scan(
        &[a.clone(), b.clone()],
        &Options {
            max_entries: 1,
            ..Options::default()
        },
        || false,
    );
    assert!(!refused.complete());
    assert_eq!(refused.files_observed, 0);
    let fresh = scan(
        &[b.clone(), a.clone(), a.clone()],
        &Options {
            max_entries: 2,
            ..Options::default()
        },
        || false,
    );
    assert!(fresh.complete(), "{fresh:?}");
    assert_eq!(fresh.files_observed, 2);
    assert_eq!(fresh.groups.len(), 1);
    assert_eq!(fresh.groups[0].paths, vec![a, b]);
}

#[cfg(unix)]
#[test]
fn whitespace_path_names_preserve_all_authored_copy_partitions() {
    use std::collections::BTreeSet;
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    fs::create_dir(&left).unwrap();
    fs::create_dir(&right).unwrap();
    let names = [
        "plain", " plain", "plain ", "\tplain", "plain\t", "plain\n", "\nplain", "plain\r",
    ];
    let mut expected = BTreeSet::new();
    for (family, name) in names.iter().enumerate() {
        let paths = [left.join(name), right.join(name)];
        let mut data = vec![19; 512];
        data[256] = family as u8;
        for path in &paths {
            fs::write(path, &data).unwrap();
        }
        let mut paths = paths.to_vec();
        paths.sort();
        expected.insert(paths);
    }
    for roots in [vec![left.clone(), right.clone(), left.clone()], vec![right, left]] {
        for verified in [false, true] {
            let report = if verified {
                rrrah_dedup::exact::scan_verified(&roots, &Options::default(), || false)
            } else {
                scan(&roots, &Options::default(), || false)
            };
            assert!(report.complete(), "{report:?}");
            assert_eq!(report.files_observed, 16);
            let actual = report
                .groups
                .iter()
                .map(|group| {
                    assert_eq!(group.bytes, 512);
                    let mut paths = group.paths.clone();
                    paths.sort();
                    paths
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn verified_scan_final_callback_cannot_publish_a_changed_earlier_member() {
    check_final_exact_callback(true);
}

#[test]
fn ordinary_scan_final_callback_cannot_publish_a_changed_earlier_member() {
    check_final_exact_callback(false);
}

fn check_final_exact_callback(verified: bool) {
    let directory = tempfile::tempdir().unwrap();
    let roots = [directory.path().join("a.bin"), directory.path().join("b.bin")];
    let original = b"same full content";
    for path in &roots {
        fs::write(path, original).unwrap();
    }
    let run = |cancel: &dyn Fn() -> bool| {
        if verified {
            rrrah_dedup::exact::scan_verified(&roots, &Options::default(), cancel)
        } else {
            scan(&roots, &Options::default(), cancel)
        }
    };
    let checkpoints = Cell::new(0_usize);
    let healthy = run(&|| {
        checkpoints.set(checkpoints.get() + 1);
        false
    });
    assert!(healthy.complete(), "{healthy:?}");
    assert_eq!(healthy.groups.len(), 1);
    let calls = Cell::new(0_usize);
    let changed = run(&|| {
        calls.set(calls.get() + 1);
        if calls.get() == checkpoints.get() {
            // The final callback runs while validating the later member.
            fs::write(&roots[0], b"changed full data").unwrap();
        }
        false
    });
    assert_eq!(calls.get(), checkpoints.get());
    assert!(changed.groups.is_empty(), "stale exact group: {changed:?}");
    assert!(changed.issues.iter().any(|issue| issue.path == roots[0]), "{changed:?}");
    fs::write(&roots[0], original).unwrap();
    let retry = run(&|| false);
    assert!(retry.complete(), "{retry:?}");
    assert_eq!(retry.groups.len(), 1);
}

#[test]
fn late_exact_source_failures_preserve_healthy_members_and_other_groups() {
    use std::collections::BTreeSet;
    for verified in [false, true] {
        for reverse in [false, true] {
            for remove in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let paths: Vec<_> = ["a", "b", "c", "d", "e"]
                    .iter().map(|name| directory.path().join(name)).collect();
                let reset = || {
                    for (i, path) in paths.iter().enumerate() {
                        fs::write(path, if i < 3 { b"first family" } else { b"other family" }).unwrap();
                    }
                };
                reset();
                let mut roots = paths.clone();
                if reverse { roots.reverse(); }
                let run = |cancel: &dyn Fn() -> bool| {
                    if verified {
                        rrrah_dedup::exact::scan_verified(&roots, &Options::default(), cancel)
                    } else {
                        scan(&roots, &Options::default(), cancel)
                    }
                };
                let checkpoints = Cell::new(0_usize);
                let baseline = run(&|| {
                    checkpoints.set(checkpoints.get() + 1);
                    false
                });
                assert!(baseline.complete(), "{baseline:?}");
                assert_eq!(baseline.groups.len(), 2);
                let calls = Cell::new(0_usize);
                let changed = run(&|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == checkpoints.get() {
                        if remove {
                            fs::remove_file(&paths[0]).unwrap();
                        } else {
                            let mtime = fs::metadata(&paths[0]).unwrap().modified().unwrap();
                            fs::write(&paths[0], b"edited bytes").unwrap();
                            fs::OpenOptions::new().write(true).open(&paths[0]).unwrap()
                                .set_times(fs::FileTimes::new().set_modified(mtime)).unwrap();
                            assert_eq!(fs::metadata(&paths[0]).unwrap().modified().unwrap(), mtime);
                        }
                    }
                    false
                });
                assert_eq!(calls.get(), checkpoints.get());
                assert!(!changed.complete(), "{changed:?}");
                assert!(!changed.cancelled);
                assert_eq!(changed.issues.len(), 1, "{changed:?}");
                assert_eq!(changed.issues[0].path, paths[0]);
                assert!(matches!(changed.issues[0].kind, IssueKind::Changed | IssueKind::Io(_)));
                let groups: BTreeSet<_> = changed.groups.iter().map(|g| g.paths.clone()).collect();
                assert_eq!(groups, BTreeSet::from([paths[1..3].to_vec(), paths[3..5].to_vec()]));
                reset();
                let retry = run(&|| false);
                assert!(retry.complete(), "{retry:?}");
                assert_eq!(retry.groups, baseline.groups);
            }
        }
    }
}

#[test]
fn atomic_cancel_exact_apis_preserve_groups_refuse_pre_cancel_and_allow_retry() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let directory = tempfile::tempdir().unwrap();
    let paths = [directory.path().join("a"), directory.path().join("b")];
    for path in &paths { fs::write(path, vec![13; 200_000]).unwrap(); }
    let flag = AtomicBool::new(false);
    for verified in [false, true] {
        let run = |roots: &[std::path::PathBuf]| {
            if verified {
                rrrah_dedup::exact::scan_verified_with_cancel_flag(roots, &Options::default(), &flag)
            } else {
                rrrah_dedup::exact::scan_with_cancel_flag(roots, &Options::default(), &flag)
            }
        };
        let healthy = run(&paths);
        assert!(healthy.complete(), "{healthy:?}");
        assert_eq!(healthy.groups.len(), 1);
        flag.store(true, Ordering::Release);
        for roots in [&paths[..], &[][..]] {
            let cancelled = run(roots);
            assert!(cancelled.cancelled, "{cancelled:?}");
            assert!(cancelled.groups.is_empty());
        }
        flag.store(false, Ordering::Release);
        let retry = run(&paths);
        assert!(retry.complete(), "{retry:?}");
        assert_eq!(retry.groups, healthy.groups);
    }
}
