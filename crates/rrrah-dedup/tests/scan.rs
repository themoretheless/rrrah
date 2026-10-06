#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::FingerprintPolicy,
    scan::{ScanError, VisualPolicy, scan_visual},
};

#[test]
fn nested_visual_roots_preserve_all_copy_edges_and_corrupt_file_attribution() {
    use rrrah_dedup::{exact::Options, scan::scan_visual_roots};
    use std::collections::{BTreeMap, BTreeSet};
    let dir = tempfile::tempdir().unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-projective");
    let mut expected = BTreeSet::new();
    let mut roots = vec![dir.path().to_path_buf()];
    for family in [1425, 2414, 2418, 2883, 5025, 5495] {
        let mut paths = Vec::new();
        for copy in 0..3 {
            let folder = dir.path().join(format!("branch-{copy}/nested"));
            std::fs::create_dir_all(&folder).unwrap();
            let path = folder.join(format!("{family}.png"));
            std::fs::copy(fixtures.join(format!("{family}-perspective.png")), &path).unwrap();
            paths.push(path);
            roots.push(folder);
        }
        paths.sort();
        for left in 0..3 {
            for right in left + 1..3 {
                expected.insert((paths[left].clone(), paths[right].clone()));
            }
        }
    }
    let broken = dir.path().join("corrupt.png");
    std::fs::write(&broken, b"not an image").unwrap();
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 1,
            max_pixels: 110_000,
            max_file_bytes: 1_000_000,
            max_cache_entries: 32,
        },
        max_files: 32,
        max_pairs: 200,
        radius: 0,
        allow_transforms: false,
        require_information: true,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    for _ in 0..2 {
        let report =
            scan_visual_roots(&roots, &Options::default(), policy, &budget, &mut cache, || false).unwrap();
        assert!(report.traversal_issues.is_empty());
        assert_eq!(report.files.len(), 19);
        assert_eq!(report.visual.analysed.len(), 18);
        let files: BTreeMap<_, _> = report.files.iter().cloned().collect();
        assert_eq!(report.visual.issues.len(), 1);
        assert_eq!(files[&report.visual.issues[0].id], broken);
        let actual: BTreeSet<_> = report
            .visual
            .pairs
            .iter()
            .map(|pair| {
                let mut paths = [files[&pair.left].clone(), files[&pair.right].clone()];
                paths.sort();
                (paths[0].clone(), paths[1].clone())
            })
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(budget.used(), 0);
        roots.reverse();
    }
}

#[test]
fn batch_errors_cache_and_pair_evidence_are_integrated() {
    let dir = tempfile::tempdir().unwrap();
    // BMP: two pixels with different luminance provide nonuniform evidence.
    let mut bytes = vec![0; 62];
    bytes[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, 62_u32), (10, 54), (14, 40), (18, 2), (22, 1), (34, 8)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[57..60].fill(255);
    let a = dir.path().join("a.bmp");
    let b = dir.path().join("b.bmp");
    let missing = dir.path().join("missing.bmp");
    std::fs::write(&a, &bytes).unwrap();
    std::fs::write(&b, &bytes).unwrap();
    let requests = vec![
        (3, DecodeRequest::new(missing)),
        (2, DecodeRequest::new(&b)),
        (1, DecodeRequest::new(&a)),
    ];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [1; 32],
            max_file_bytes: 1024,
            max_pixels: 2,
            max_frames: 10,
            max_cache_entries: 10,
        },
        max_files: 10,
        max_pairs: 10,
        radius: 0,
        allow_transforms: true,
        require_information: true,
    };
    let budget = MemoryBudget::new(1024 * 1024);
    let mut cache = FingerprintCache::default();
    let first = scan_visual(requests.clone(), policy, &budget, &mut cache, || false).unwrap();
    assert_eq!(first.analysed, [1, 2]);
    assert_eq!(first.issues.len(), 1);
    assert_eq!(first.issues[0].id, 3);
    assert_eq!(first.pairs.len(), 1);
    assert_eq!((first.pairs[0].left, first.pairs[0].right), (1, 2));
    assert!(first.pairs[0].evidence.informative);
    assert_eq!(first.cache_hits, 1);
    let confirmed = rrrah_dedup::scan::confirm_pixels(
        requests.clone(),
        [(2, 1), (1, 2), (1, 3)],
        policy,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(confirmed.equal, [(1, 2)]);
    assert!(confirmed.different.is_empty());
    assert_eq!(confirmed.issues.len(), 1);
    assert_eq!((confirmed.issues[0].left, confirmed.issues[0].right), (1, 3));
    assert_eq!(budget.used(), 0);
    check_confirmed_grouping(dir.path(), &requests, &bytes, policy, &budget);
    let mut changed = bytes.clone();
    changed[57] = 0;
    std::fs::write(&b, changed).unwrap();
    let negative =
        rrrah_dedup::scan::confirm_pixels(requests.clone(), [(1, 2)], policy, &budget, || false).unwrap();
    assert_eq!(negative.different, [(1, 2)]);
    assert!(negative.equal.is_empty());
    assert!(matches!(
        rrrah_dedup::scan::confirm_pixels(requests.clone(), [(1, 1)], policy, &budget, || false),
        Err(ScanError::InvalidPair)
    ));
    assert!(matches!(
        rrrah_dedup::scan::confirm_pixels(requests.clone(), [(1, 2)], policy, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    std::fs::write(&b, &bytes).unwrap();
    let mut reordered = requests.clone();
    reordered.reverse();
    let second = scan_visual(reordered, policy, &budget, &mut cache, || false).unwrap();
    assert_eq!(second.cache_hits, 2);
    assert_eq!(second.pairs, first.pairs);
    assert!(matches!(
        scan_visual(
            requests.clone(),
            VisualPolicy {
                max_pairs: 0,
                ..policy
            },
            &budget,
            &mut cache,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert!(matches!(
        scan_visual(requests, policy, &budget, &mut cache, || true),
        Err(ScanError::Cancelled)
    ));
}

#[test]
fn confirmation_mutation_generation_and_memory_are_inconclusive() {
    use rrrah_dedup::scan::{ConfirmationError, confirm_pixels};
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    std::fs::copy(root, &a).unwrap();
    std::fs::copy(&a, &b).unwrap();
    let files = vec![(1, DecodeRequest::new(&a)), (2, DecodeRequest::new(&b))];
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: 100_000,
            max_pixels: 100,
            max_frames: 10,
            max_cache_entries: 0,
        },
        max_files: 2,
        max_pairs: 1,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mutated = Cell::new(false);
    let report = confirm_pixels(files.clone(), [(1, 2)], policy, &budget, || {
        // Managed normalized frames exist: mutate metadata without changing pixels.
        if budget.used() > 0 && !mutated.replace(true) {
            let mut bytes = std::fs::read(&a).unwrap();
            bytes.push(0);
            std::fs::write(&a, bytes).unwrap();
        }
        false
    })
    .unwrap();
    assert!(mutated.get());
    assert!(report.equal.is_empty());
    assert!(report.different.is_empty());
    assert!(matches!(
        report.issues[0].error,
        ConfirmationError::Prepared(rrrah_dedup::decode::CachedError::Source(
            rrrah_dedup::exact::SnapshotError::Changed
        ))
    ));
    assert_eq!(budget.used(), 0);
    std::fs::copy(&b, &a).unwrap();
    let denied = MemoryBudget::new(0);
    let report = confirm_pixels(files.clone(), [(1, 2)], policy, &denied, || false).unwrap();
    assert!(report.equal.is_empty());
    assert_eq!(report.issues.len(), 1);
    assert_eq!(denied.used(), 0);
    let generation = Arc::new(AtomicU64::new(7));
    let mut stale = files;
    stale[0].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    let result = confirm_pixels(stale, [(1, 2)], policy, &budget, || {
        if budget.used() > 0 {
            generation.store(8, Ordering::SeqCst);
        }
        false
    });
    assert!(
        matches!(result, Err(ScanError::Cancelled)),
        "{result:?}, generation {}",
        generation.load(Ordering::SeqCst)
    );
    assert_eq!(budget.used(), 0);
}

fn check_confirmed_grouping(
    directory: &std::path::Path,
    requests: &[(u64, DecodeRequest)],
    bytes: &[u8],
    policy: VisualPolicy,
    budget: &MemoryBudget,
) {
    let c = directory.join("c.bmp");
    std::fs::write(&c, bytes).unwrap();
    let mut group_requests = requests.to_vec();
    group_requests.push((4, DecodeRequest::new(c)));
    let group_budget = rrrah_dedup::groups::GroupBudget {
        max_entries: 10,
        max_pairs: 10,
        max_pair_checks: 100,
    };
    let chain = rrrah_dedup::scan::confirm_pixel_groups(
        group_requests.clone(),
        [(1, 2), (2, 4), (1, 3)],
        policy,
        group_budget,
        budget,
        || false,
    )
    .unwrap();
    assert_eq!(chain.grouping.groups, [vec![1, 2], vec![3], vec![4]]);
    assert_eq!(chain.grouping.pairs.len(), 2);
    assert_eq!(chain.confirmation.issues.len(), 1);
    let complete = rrrah_dedup::scan::confirm_pixel_groups(
        group_requests,
        [(1, 2), (2, 4), (1, 4)],
        policy,
        group_budget,
        budget,
        || false,
    )
    .unwrap();
    assert_eq!(complete.grouping.groups, [vec![1, 2, 4], vec![3]]);
    assert!(matches!(
        rrrah_dedup::scan::confirm_pixels(requests.to_vec(), [(1, 2); 11], policy, budget, || false),
        Err(ScanError::Budget)
    ));
}

#[test]
fn tiff_page_fingerprints_and_confirmation_share_profile_and_limits() {
    use rrrah_dedup::{decode::fingerprint_file, scan::confirm_pixels};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let mut a = DecodeRequest::new(root.join("classic-little-none-same.tif"));
    a.image_index = 2;
    let mut b = DecodeRequest::new(root.join("bigtiff-big-lzw-same.tif"));
    b.image_index = 2;
    let mut c = DecodeRequest::new(root.join("bigtiff-big-lzw-changed.tif"));
    c.image_index = 2;
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 3,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 10,
        },
        max_files: 3,
        max_pairs: 3,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let files = vec![(1, a.clone()), (2, b), (3, c)];
    let visual = scan_visual(files.clone(), policy, &budget, &mut cache, || false).unwrap();
    assert_eq!(visual.analysed, [1, 2, 3]);
    assert!(visual.issues.is_empty());
    let report = confirm_pixels(files, [(1, 2), (1, 3)], policy, &budget, || false).unwrap();
    assert_eq!(report.equal, [(1, 2)]);
    assert_eq!(report.different, [(1, 3)]);
    assert!(
        fingerprint_file(&a, policy.fingerprint, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        fingerprint_file(
            &a,
            FingerprintPolicy {
                max_frames: 2,
                ..policy.fingerprint
            },
            &budget,
            &mut cache,
            || false
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_visual_discovery_keeps_aliases_and_errors_visible() {
    use rrrah_dedup::{exact::Options, scan::scan_visual_roots};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let a = dir.path().join("a.png");
    let b = dir.path().join("nested/b.png");
    std::fs::copy(source, &a).unwrap();
    std::fs::copy(&a, &b).unwrap();
    std::fs::write(dir.path().join("broken.bin"), b"not an image").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(&a, dir.path().join("alias.png")).unwrap();
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 10,
        },
        max_files: 10,
        max_pairs: 10,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let report = scan_visual_roots(
        &[dir.path().into()],
        &Options::default(),
        policy,
        &budget,
        &mut cache,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.visual.analysed.len(), 2);
    assert_eq!(report.visual.issues.len(), 1);
    assert_eq!(report.visual.pairs.len(), 1);
    #[cfg(unix)]
    assert_eq!(report.aliases.len(), 1);
    let shallow = scan_visual_roots(
        &[dir.path().into()],
        &Options {
            max_depth: 1,
            ..Options::default()
        },
        policy,
        &budget,
        &mut cache,
        || false,
    )
    .unwrap();
    assert!(
        shallow
            .traversal_issues
            .iter()
            .any(|issue| issue.kind == rrrah_dedup::exact::IssueKind::Limit)
    );
    assert!(shallow.visual.pairs.is_empty());
    assert!(matches!(
        scan_visual_roots(
            &[dir.path().into()],
            &Options::default(),
            policy,
            &budget,
            &mut cache,
            || true
        ),
        Err(ScanError::Cancelled)
    ));
}

#[test]
fn group_admission_invalidates_edges_from_changed_source() {
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let c = dir.path().join("c.png");
    std::fs::copy(fixture, &a).unwrap();
    std::fs::copy(&a, &b).unwrap();
    std::fs::copy(&a, &c).unwrap();
    let policy = VisualPolicy {
        fingerprint: FingerprintPolicy {
            recipe: [0; 32],
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
            max_cache_entries: 0,
        },
        max_files: 3,
        max_pairs: 3,
        radius: 0,
        allow_transforms: false,
        require_information: false,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let active = Cell::new(false);
    let changed = Cell::new(false);
    let report = rrrah_dedup::scan::confirm_pixel_groups(
        [
            (1, DecodeRequest::new(&a)),
            (2, DecodeRequest::new(&b)),
            (3, DecodeRequest::new(&c)),
        ],
        [(1, 2), (1, 3), (2, 3)],
        policy,
        rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 100,
        },
        &budget,
        || {
            if budget.used() > 0 {
                active.set(true);
            } else if active.get() && !changed.replace(true) {
                // The first comparison released its rasters: change its first source.
                let mut bytes = std::fs::read(&a).unwrap();
                bytes.push(0);
                std::fs::write(&a, bytes).unwrap();
            }
            false
        },
    )
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.confirmation.equal, [(2, 3)]);
    assert_eq!(report.grouping.groups, [vec![1], vec![2, 3]]);
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 1);
    assert_eq!(budget.used(), 0);
}

#[test]
fn pixel_group_final_checks_observe_generation_cancellation() {
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline/base.png");
    let mut files = vec![(1, DecodeRequest::new(&path)), (2, DecodeRequest::new(&path))];
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
    let groups = rrrah_dedup::groups::GroupBudget {
        max_entries: 2,
        max_pairs: 1,
        max_pair_checks: 10,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let calls = Cell::new(0);
    rrrah_dedup::scan::confirm_pixel_groups(files.clone(), [(1, 2)], policy, groups, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let stop = calls.get() - 1;
    let generation = Arc::new(AtomicU64::new(7));
    files[0].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    calls.set(0);
    let result = rrrah_dedup::scan::confirm_pixel_groups(files, [(1, 2)], policy, groups, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == stop {
            generation.store(8, Ordering::Release);
        }
        false
    });
    assert_eq!(generation.load(Ordering::Acquire), 8);
    assert!(
        matches!(
            result,
            Err(ScanError::Cancelled | ScanError::Group(rrrah_dedup::groups::GroupError::Cancelled))
        ),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
}
