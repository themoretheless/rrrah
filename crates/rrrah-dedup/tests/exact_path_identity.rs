//! POSIX backslashes are filename bytes, not directory separators.
#![cfg(unix)]

use rrrah_dedup::exact::{Options, scan, scan_verified};
use std::{collections::BTreeSet, fs};

#[test]
fn literal_backslash_and_directory_separator_keep_independent_copy_groups() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("a")).unwrap();
    let paths = [
        root.path().join("a\\payload.txt"),
        root.path().join("a/payload.txt"),
        root.path().join("backslash-copy"),
        root.path().join("nested-copy"),
    ];
    for (path, bytes) in paths.iter().zip([b"AAAA", b"BBBB", b"AAAA", b"BBBB"]) {
        fs::write(path, bytes).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))
            .unwrap();
    }
    use std::os::unix::fs::MetadataExt;
    assert_ne!(
        fs::metadata(&paths[0]).unwrap().ino(),
        fs::metadata(&paths[1]).unwrap().ino()
    );
    let expected = BTreeSet::from([
        BTreeSet::from([paths[0].clone(), paths[2].clone()]),
        BTreeSet::from([paths[1].clone(), paths[3].clone()]),
    ]);
    for reverse in [false, true] {
        let mut roots = paths.to_vec();
        roots.push(root.path().to_path_buf());
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
            assert_eq!(report.files_observed, 4);
            let actual: BTreeSet<BTreeSet<_>> = report
                .groups
                .into_iter()
                .map(|group| group.paths.into_iter().collect())
                .collect();
            assert_eq!(actual, expected);
        }
    }
}
