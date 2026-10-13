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
