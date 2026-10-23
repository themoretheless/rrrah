use rrrah_dedup::exact::{Options, compare_files, scan, scan_verified};
use std::fs;

#[test]
fn repeated_comparisons_reread_content_when_size_and_modified_time_are_restored() {
    for size in [4, 261, 4096, 65_537] {
        let directory = tempfile::tempdir().unwrap();
        let paths: Vec<_> = ["a", "b", "c"]
            .map(|name| directory.path().join(name))
            .into_iter()
            .collect();
        let bytes = vec![37; size];
        for path in &paths {
            fs::write(path, &bytes).unwrap();
        }
        for verified in [false, true] {
            fs::write(&paths[1], &bytes).unwrap();
            let before = fs::metadata(&paths[1]).unwrap();
            let modified = before.modified().unwrap();
            assert!(compare_files(&paths[0], &paths[1], size as u64, || false).unwrap());
            let run = || {
                if verified {
                    scan_verified(&paths, &Options::default(), || false)
                } else {
                    scan(&paths, &Options::default(), || false)
                }
            };
            let equal = run();
            assert!(equal.complete(), "{equal:?}");
            assert_eq!(equal.groups.len(), 1);
            assert_eq!(equal.groups[0].paths.len(), 3);

            let mut different = bytes.clone();
            different[size - 1] ^= 1;
            fs::write(&paths[1], different).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&paths[1])
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            let after = fs::metadata(&paths[1]).unwrap();
            assert_eq!(after.len(), before.len());
            assert_eq!(after.modified().unwrap(), modified);
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
            }
            assert!(!compare_files(&paths[0], &paths[1], size as u64, || false).unwrap());
            assert!(!compare_files(&paths[1], &paths[0], size as u64, || false).unwrap());
            assert!(compare_files(&paths[0], &paths[2], size as u64, || false).unwrap());
            let changed = run();
            assert!(changed.complete(), "{changed:?}");
            assert_eq!(changed.groups.len(), 1);
            assert_eq!(changed.groups[0].paths, vec![paths[0].clone(), paths[2].clone()]);
        }
    }
}

#[test]
fn rescans_follow_renames_removal_and_recreated_paths() {
    for verified in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let a = root.join("a");
        let b = root.join("b");
        let moved = root.join("moved");
        fs::create_dir(&moved).unwrap();
        let moved_a = moved.join("a");
        let moved_b = moved.join("b");
        fs::write(&a, b"same-a").unwrap();
        fs::write(&b, b"same-a").unwrap();
        let check = |expected: &[std::path::PathBuf], observed| {
            let inputs = [root.to_path_buf()];
            let report = if verified {
                scan_verified(&inputs, &Options::default(), || false)
            } else {
                scan(&inputs, &Options::default(), || false)
            };
            assert!(report.complete(), "{report:?}");
            assert!(report.issues.is_empty(), "{report:?}");
            assert_eq!(report.files_observed, observed);
            if expected.is_empty() {
                assert!(report.groups.is_empty(), "{report:?}");
            } else {
                assert_eq!(report.groups.len(), 1, "{report:?}");
                let mut expected = expected.to_vec();
                expected.sort();
                assert_eq!(report.groups[0].paths, expected);
                assert_eq!(report.groups[0].bytes, 6);
            }
        };
        check(&[a.clone(), b.clone()], 2);
        fs::rename(&b, &moved_b).unwrap();
        fs::write(&b, b"same-b").unwrap();
        check(&[a.clone(), moved_b.clone()], 3);
        fs::rename(&a, &moved_a).unwrap();
        fs::remove_file(&moved_b).unwrap();
        check(&[], 2);
        fs::write(&moved_b, b"same-b").unwrap();
        check(&[b, moved_b], 3);
    }
}
