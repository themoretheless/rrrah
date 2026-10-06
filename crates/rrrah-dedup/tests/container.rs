#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::{AnimationBudget, AnimationKind},
    container::{ContainerError, Presentation, same_presentation},
};

#[test]
fn selected_frame_does_not_establish_whole_animation_identity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let base = DecodeRequest::new(root.join("base.gif"));
    let split = DecodeRequest::new(root.join("split.apng"));
    let changed = DecodeRequest::new(root.join("changed-pixels.apng"));
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    assert!(
        same_presentation(
            (&base, Presentation::SelectedFrame),
            (&changed, Presentation::SelectedFrame),
            limits,
            &budget,
            || false
        )
        .unwrap()
    );
    let gif = Presentation::Animation(AnimationKind::Gif);
    let apng = Presentation::Animation(AnimationKind::Apng);
    assert!(!same_presentation((&base, gif), (&changed, apng), limits, &budget, || false).unwrap());
    assert!(same_presentation((&base, gif), (&split, apng), limits, &budget, || false).unwrap());
    assert!(matches!(
        same_presentation(
            (&base, gif),
            (&split, Presentation::SelectedFrame),
            limits,
            &budget,
            || false
        ),
        Err(ContainerError::Incompatible)
    ));
    assert!(matches!(
        same_presentation((&base, gif), (&split, apng), limits, &budget, || true),
        Err(ContainerError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn complete_page_comparison_rejects_last_page_changes_and_limits() {
    use rrrah_dedup::pages::PageKind;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let base = DecodeRequest::new(root.join("classic-little-none-same.tif"));
    let same = DecodeRequest::new(root.join("bigtiff-big-lzw-same.tif"));
    let changed = DecodeRequest::new(root.join("bigtiff-big-lzw-changed.tif"));
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let pages = Presentation::Pages(PageKind::Tiff);
    assert!(same_presentation((&base, pages), (&same, pages), limits, &budget, || false).unwrap());
    assert!(!same_presentation((&base, pages), (&changed, pages), limits, &budget, || false).unwrap());
    assert!(
        same_presentation(
            (&base, Presentation::SelectedFrame),
            (&changed, Presentation::SelectedFrame),
            limits,
            &budget,
            || false
        )
        .unwrap()
    );
    assert!(
        same_presentation(
            (&base, pages),
            (&same, pages),
            AnimationBudget {
                max_frames: 2,
                ..limits
            },
            &budget,
            || false
        )
        .is_err()
    );
    assert!(
        same_presentation(
            (&base, pages),
            (&same, pages),
            limits,
            &MemoryBudget::new(0),
            || false
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn dcx_icon_and_cursor_page_modes_use_existing_full_decoders() {
    use rrrah_dedup::pages::PageKind;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    for (name, kind) in [
        ("two-pages.dcx", PageKind::Dcx),
        ("pattern.ico", PageKind::Icon),
        ("pattern.ico.cur", PageKind::Icon),
    ] {
        let request = DecodeRequest::new(root.join(name));
        assert!(
            same_presentation(
                (&request, Presentation::Pages(kind)),
                (&request, Presentation::Pages(kind)),
                limits,
                &budget,
                || false
            )
            .unwrap(),
            "{name}"
        );
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn batch_full_presentations_preserve_negative_and_error_evidence() {
    use rrrah_dedup::{container::confirm_presentations, scan::ScanError};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let files = vec![
        (
            1,
            DecodeRequest::new(root.join("base.gif")),
            Presentation::Animation(AnimationKind::Gif),
        ),
        (
            2,
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
        (
            3,
            DecodeRequest::new(root.join("changed-pixels.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
        (
            4,
            DecodeRequest::new(root.join("missing.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
    ];
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let report = confirm_presentations(
        files.clone(),
        [(2, 1), (1, 2), (1, 3), (1, 4)],
        4,
        4,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.equal, [(1, 2)]);
    assert_eq!(report.different, [(1, 3)]);
    assert_eq!(report.issues.len(), 1);
    assert_eq!((report.issues[0].left, report.issues[0].right), (1, 4));
    assert_eq!(budget.used(), 0);
    let exhaustive =
        rrrah_dedup::container::scan_presentations(files.clone(), 4, 6, limits, &budget, || false).unwrap();
    assert_eq!(exhaustive.equal, [(1, 2)]);
    assert_eq!(exhaustive.different, [(1, 3), (2, 3)]);
    assert_eq!(
        exhaustive
            .issues
            .iter()
            .map(|issue| (issue.left, issue.right))
            .collect::<Vec<_>>(),
        [(1, 4), (2, 4), (3, 4)]
    );
    assert!(matches!(
        rrrah_dedup::container::scan_presentations(files.clone(), 4, 5, limits, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert!(matches!(
        confirm_presentations(files.clone(), [(1, 2); 5], 4, 4, limits, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert!(matches!(
        confirm_presentations(files, [(1, 2)], 4, 4, limits, &budget, || true),
        Err(ScanError::Cancelled)
    ));
}

#[test]
fn exhaustive_scan_does_not_filter_on_zero_duration_first_frame() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let a = DecodeRequest::new(root.join("base.gif"));
    let b = DecodeRequest::new(root.join("zero-prefix.apng"));
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    assert!(
        !same_presentation(
            (&a, Presentation::SelectedFrame),
            (&b, Presentation::SelectedFrame),
            limits,
            &budget,
            || false
        )
        .unwrap()
    );
    let report = rrrah_dedup::container::scan_presentations(
        [
            (1, a, Presentation::Animation(AnimationKind::Gif)),
            (2, b, Presentation::Animation(AnimationKind::Apng)),
        ],
        2,
        1,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.equal, [(1, 2)]);
    assert!(report.issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
fn complete_animation_groups_remove_stale_edges_and_keep_healthy_group() {
    use rrrah_dedup::container::{PresentationPolicy, scan_presentation_groups};
    use std::cell::Cell;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.gif");
    std::fs::copy(root.join("base.gif"), &a).unwrap();
    let files = vec![
        (
            1,
            DecodeRequest::new(&a),
            Presentation::Animation(AnimationKind::Gif),
        ),
        (
            2,
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
        (
            3,
            DecodeRequest::new(root.join("zero-prefix.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
    ];
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 100,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let stable = scan_presentation_groups(files.clone(), policy, &budget, || false).unwrap();
    assert_eq!(stable.grouping.groups, [vec![1, 2, 3]]);
    assert!(stable.source_issues.is_empty());
    let active = Cell::new(false);
    let changed = Cell::new(false);
    let result = scan_presentation_groups(files, policy, &budget, || {
        if budget.used() > 0 {
            active.set(true);
        } else if active.get() && !changed.replace(true) {
            let mut bytes = std::fs::read(&a).unwrap();
            bytes.push(0);
            std::fs::write(&a, bytes).unwrap();
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert_eq!(result.grouping.groups, [vec![1], vec![2, 3]]);
    assert_eq!(result.comparisons.equal, [(2, 3)]);
    assert_eq!(result.source_issues.len(), 1);
    assert_eq!(result.source_issues[0].0, 1);
    assert_eq!(budget.used(), 0);
}

#[test]
fn generation_change_at_final_group_admission_returns_no_result() {
    use rrrah_dedup::container::{PresentationPolicy, scan_presentation_groups};
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let files = vec![
        (
            1,
            DecodeRequest::new(root.join("base.gif")),
            Presentation::Animation(AnimationKind::Gif),
        ),
        (
            2,
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        ),
    ];
    let policy = PresentationPolicy {
        max_files: 2,
        max_pairs: 1,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 2,
            max_pairs: 1,
            max_pair_checks: 10,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let calls = Cell::new(0);
    scan_presentation_groups(files.clone(), policy, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let stop = calls.get() - 1;
    assert!(stop > 10);
    let generation = Arc::new(AtomicU64::new(7));
    let mut files = files;
    files[0].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    calls.set(0);
    let result = scan_presentation_groups(files, policy, &budget, || {
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
            Err(rrrah_dedup::scan::ScanError::Cancelled
                | rrrah_dedup::scan::ScanError::Group(rrrah_dedup::groups::GroupError::Cancelled))
        ),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_complete_animation_groups_preserve_paths_aliases_and_errors() {
    use rrrah_dedup::container::{PresentationPolicy, scan_presentation_roots};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::copy(root.join("base.gif"), dir.path().join("a.gif")).unwrap();
    std::fs::copy(root.join("split.apng"), dir.path().join("nested/b.apng")).unwrap();
    std::fs::copy(
        root.join("changed-pixels.apng"),
        dir.path().join("nested/changed.apng"),
    )
    .unwrap();
    std::fs::write(dir.path().join("broken.bin"), b"broken image").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(dir.path().join("a.gif"), dir.path().join("alias.gif")).unwrap();
    let policy = PresentationPolicy {
        max_files: 4,
        max_pairs: 6,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 4,
            max_pairs: 6,
            max_pair_checks: 100,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let choose = |path: &std::path::Path| match path.extension().and_then(|v| v.to_str()) {
        Some("gif") => Presentation::Animation(AnimationKind::Gif),
        Some("apng") => Presentation::Animation(AnimationKind::Apng),
        _ => Presentation::SelectedFrame,
    };
    let report = scan_presentation_roots(
        &[dir.path().into()],
        &rrrah_dedup::exact::Options::default(),
        choose,
        policy,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.result.grouping.groups, [vec![0, 2], vec![1], vec![3]]);
    assert_eq!(report.result.comparisons.equal, [(0, 2)]);
    assert_eq!(report.result.comparisons.issues.len(), 3);
    assert!(report.traversal_issues.is_empty());
    #[cfg(unix)]
    assert_eq!(report.aliases.len(), 1);
    assert_eq!(budget.used(), 0);
}

#[test]
fn automatic_directory_search_groups_renamed_containers_and_retains_detection_failures() {
    use rrrah_dedup::container::{PresentationPolicy, scan_presentation_roots_auto};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    for (source, name) in [
        ("base.gif", "a.bin"),
        ("split.apng", "b.bin"),
        ("changed-pixels.apng", "c.bin"),
    ] {
        std::fs::copy(root.join(source), dir.path().join(name)).unwrap();
    }
    // Structurally impossible PNG chunk length, classified as a detection failure.
    std::fs::write(
        dir.path().join("d.bin"),
        b"\x89PNG\r\n\x1a\n\xff\xff\xff\xffIHDR\0\0\0\0",
    )
    .unwrap();
    let policy = PresentationPolicy {
        max_files: 4,
        max_pairs: 6,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 4,
            max_pairs: 6,
            max_pair_checks: 100,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let report = scan_presentation_roots_auto(
        &[dir.path().into()],
        &rrrah_dedup::exact::Options::default(),
        4096,
        policy,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.result.grouping.groups, [vec![0, 1], vec![2], vec![3]]);
    assert_eq!(report.result.comparisons.equal, [(0, 1)]);
    assert_eq!(report.detection_issues.len(), 1);
    assert_eq!(report.detection_issues[0].0, 3);
    assert!(report.files[3].2.is_none());
    assert!(report.traversal_issues.is_empty());
    assert_eq!(budget.used(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // One labelled fixture qualifies both routes and every cancellation checkpoint.
fn indexed_recursive_modes_retain_labelled_groups_with_candidate_only_budget() {
    use rrrah_dedup::container::{
        PresentationPolicy, scan_indexed_presentation_roots, scan_indexed_presentation_roots_auto,
    };
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    for (source, destination) in [
        ("base.gif", "a.gif"),
        ("split.apng", "nested/b.apng"),
        ("changed-pixels.apng", "nested/c.apng"),
    ] {
        std::fs::copy(fixtures.join(source), dir.path().join(destination)).unwrap();
    }
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 1,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 1,
            max_pair_checks: 10,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let roots = [dir.path().into(), dir.path().join("nested")];
    let traversal = rrrah_dedup::exact::Options::default();
    let mode = |path: &std::path::Path| {
        Presentation::Animation(if path.extension().unwrap() == "gif" {
            AnimationKind::Gif
        } else {
            AnimationKind::Apng
        })
    };
    let explicit =
        scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, || false).unwrap();
    let automatic =
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
    for (candidates, preparations, issues, result) in [
        (
            explicit.candidate_pairs,
            explicit.signature_preparations,
            &explicit.preparation_issues,
            &explicit.result.result,
        ),
        (
            automatic.candidate_pairs,
            automatic.signature_preparations,
            &automatic.preparation_issues,
            &automatic.result.result,
        ),
    ] {
        assert_eq!(candidates, 1);
        assert_eq!(preparations, 3);
        assert!(issues.is_empty());
        assert_eq!(result.comparisons.equal, vec![(0, 1)]);
        assert_eq!(result.grouping.groups, vec![vec![0, 1], vec![2]]);
        assert!(result.source_issues.is_empty());
    }
    assert!(automatic.result.detection_issues.is_empty());
    // Independent cancellation schedule spans discovery, classification, key
    // preparation, direct confirmation, final snapshots and regrouping.
    for automatic_mode in [false, true] {
        let calls = std::cell::Cell::new(0usize);
        let run = |stop: usize| {
            let cancel = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            if automatic_mode {
                scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, cancel)
                    .map(|_| ())
            } else {
                scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, cancel).map(|_| ())
            }
        };
        run(usize::MAX).unwrap();
        let checkpoints = calls.get();
        assert!(checkpoints > 10);
        for stop in 1..=checkpoints {
            calls.set(0);
            let outcome = run(stop);
            assert!(
                matches!(
                    outcome,
                    Err(rrrah_dedup::scan::ScanError::Cancelled
                        | rrrah_dedup::scan::ScanError::Group(rrrah_dedup::groups::GroupError::Cancelled))
                ),
                "automatic={automatic_mode} stop={stop}: {outcome:?}"
            );
            assert_eq!(budget.used(), 0);
        }
        println!("recursive indexed automatic={automatic_mode} cancellation_checkpoints={checkpoints}");
    }
    let mut refused = policy;
    refused.max_pairs = 0;
    assert!(matches!(
        scan_indexed_presentation_roots(&roots, &traversal, mode, refused, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert!(matches!(
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, refused, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert!(matches!(
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || true),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_recursive_corrupt_animation_is_inconclusive_and_preserves_healthy_edges() {
    use rrrah_dedup::container::{
        PresentationPolicy, scan_indexed_presentation_roots, scan_indexed_presentation_roots_auto,
    };
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixtures.join("base.gif"), dir.path().join("a.gif")).unwrap();
    std::fs::copy(fixtures.join("split.apng"), dir.path().join("b.apng")).unwrap();
    // Recognizable GIF header, with no logical screen or frame data.
    std::fs::write(dir.path().join("c.gif"), b"GIF89a").unwrap();
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 10,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let roots = [dir.path().into()];
    let traversal = rrrah_dedup::exact::Options::default();
    let mode = |path: &std::path::Path| {
        Presentation::Animation(if path.extension().unwrap() == "gif" {
            AnimationKind::Gif
        } else {
            AnimationKind::Apng
        })
    };
    let explicit =
        scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, || false).unwrap();
    let automatic =
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
    assert!(automatic.result.detection_issues.is_empty());
    for (candidates, preparation, report) in [
        (
            explicit.candidate_pairs,
            &explicit.preparation_issues,
            &explicit.result.result,
        ),
        (
            automatic.candidate_pairs,
            &automatic.preparation_issues,
            &automatic.result.result,
        ),
    ] {
        assert_eq!(candidates, 3);
        assert_eq!(preparation.len(), 1);
        assert_eq!(preparation[0].0, 2);
        assert_eq!(report.comparisons.equal, vec![(0, 1)]);
        assert!(report.comparisons.different.is_empty());
        let failed: Vec<_> = report
            .comparisons
            .issues
            .iter()
            .map(|issue| (issue.left, issue.right))
            .collect();
        assert_eq!(failed, vec![(0, 2), (1, 2)]);
        assert_eq!(report.grouping.groups, vec![vec![0, 1], vec![2]]);
    }
    let mut small = policy;
    small.max_pairs = 1;
    assert!(matches!(
        scan_indexed_presentation_roots(&roots, &traversal, mode, small, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert!(matches!(
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, small, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    // Repair the same path: retry must not retain the old failure or omit its edges.
    std::fs::copy(fixtures.join("base.gif"), dir.path().join("c.gif")).unwrap();
    let repaired =
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
    assert!(repaired.preparation_issues.is_empty());
    assert!(repaired.result.result.comparisons.issues.is_empty());
    assert_eq!(
        repaired.result.result.comparisons.equal,
        vec![(0, 1), (0, 2), (1, 2)]
    );
    assert_eq!(repaired.result.result.grouping.groups, vec![vec![0, 1, 2]]);
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_recursive_source_change_removes_stale_edges_and_retry_restores_them() {
    use rrrah_dedup::container::{
        PresentationPolicy, scan_indexed_presentation_roots, scan_indexed_presentation_roots_auto,
    };
    use std::cell::Cell;
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    for (source, destination) in [
        ("base.gif", "a.gif"),
        ("split.apng", "b.apng"),
        ("zero-prefix.apng", "c.apng"),
    ] {
        std::fs::copy(fixtures.join(source), dir.path().join(destination)).unwrap();
    }
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 10,
        },
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let roots = [dir.path().into()];
    let traversal = rrrah_dedup::exact::Options::default();
    let mode = |path: &std::path::Path| {
        Presentation::Animation(if path.extension().unwrap() == "gif" {
            AnimationKind::Gif
        } else {
            AnimationKind::Apng
        })
    };
    let changed_path = dir.path().join("a.gif");
    for automatic in [false, true] {
        std::fs::copy(fixtures.join("base.gif"), &changed_path).unwrap();
        let active = Cell::new(false);
        let changed = Cell::new(false);
        let cancel = || {
            if budget.used() > 0 {
                active.set(true);
            } else if active.get() && !changed.replace(true) {
                // Append after the first prepared presentation is released:
                // valid visual content remains, but the observed source changed.
                let mut bytes = std::fs::read(&changed_path).unwrap();
                bytes.push(0);
                std::fs::write(&changed_path, bytes).unwrap();
            }
            false
        };
        let report = if automatic {
            scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, cancel)
                .unwrap()
                .result
                .result
        } else {
            scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, cancel)
                .unwrap()
                .result
                .result
        };
        assert!(changed.get(), "automatic={automatic}");
        assert_eq!(report.comparisons.equal, vec![(1, 2)], "automatic={automatic}");
        assert_eq!(report.grouping.groups, vec![vec![0], vec![1, 2]]);
        assert_eq!(report.source_issues.len(), 1);
        assert_eq!(report.source_issues[0].0, 0);
        assert_eq!(budget.used(), 0);
        std::fs::copy(fixtures.join("base.gif"), &changed_path).unwrap();
        let retry = if automatic {
            scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false)
                .unwrap()
                .result
                .result
        } else {
            scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, || false)
                .unwrap()
                .result
                .result
        };
        assert!(retry.source_issues.is_empty());
        assert_eq!(retry.comparisons.equal, vec![(0, 1), (0, 2), (1, 2)]);
        assert_eq!(retry.grouping.groups, vec![vec![0, 1, 2]]);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One independently labelled mixed fixture verifies both recursive routes.
fn mixed_recursive_indexed_presentations_cover_all_three_scope_classes() {
    use rrrah_dedup::{
        container::{
            PresentationPolicy, scan_indexed_presentation_roots, scan_indexed_presentation_roots_auto,
        },
        pages::PageKind,
    };
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    // Independent labels come from the fixture recipes, not scanner output.
    let sources = [
        "timeline/base.gif",
        "timeline/split.apng",
        "timeline/changed-pixels.apng",
        "tiff/classic-little-none-same.tif",
        "tiff/bigtiff-big-lzw-same.tif",
        "tiff/bigtiff-big-lzw-changed.tif",
        "raster-equality/opaque-base.png",
        "raster-equality/opaque-base.bmp",
        "raster-equality/opaque-changed.png",
    ];
    for (id, source) in sources.iter().enumerate() {
        // Every extension is intentionally misleading: automatic mode must use content.
        std::fs::copy(
            fixtures.join(source),
            dir.path().join("nested").join(format!("{id}.data")),
        )
        .unwrap();
    }
    let policy = PresentationPolicy {
        max_files: 9,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100_000,
            max_file_bytes: 1_000_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 9,
            max_pairs: 3,
            max_pair_checks: 30,
        },
    };
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let roots = [dir.path().into(), dir.path().join("nested")];
    let traversal = rrrah_dedup::exact::Options::default();
    let mode = |path: &std::path::Path| match path.file_stem().unwrap().to_str().unwrap() {
        "0" => Presentation::Animation(AnimationKind::Gif),
        "1" | "2" => Presentation::Animation(AnimationKind::Apng),
        "3" | "4" | "5" => Presentation::Pages(PageKind::Tiff),
        _ => Presentation::SelectedFrame,
    };
    let explicit =
        scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, || false).unwrap();
    let automatic =
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
    assert!(automatic.result.detection_issues.is_empty());
    assert_eq!(automatic.result.files.len(), 9);
    for (_, path, detected) in &automatic.result.files {
        assert!(
            matches!(
                (detected, mode(path)),
                (Some(Presentation::SelectedFrame), Presentation::SelectedFrame)
                    | (
                        Some(Presentation::Pages(PageKind::Tiff)),
                        Presentation::Pages(PageKind::Tiff)
                    )
                    | (
                        Some(Presentation::Animation(AnimationKind::Gif)),
                        Presentation::Animation(AnimationKind::Gif)
                    )
                    | (
                        Some(Presentation::Animation(AnimationKind::Apng)),
                        Presentation::Animation(AnimationKind::Apng)
                    )
            ),
            "wrong mode for {}: {detected:?}",
            path.display()
        );
    }
    for (candidates, preparations, errors, report) in [
        (
            explicit.candidate_pairs,
            explicit.signature_preparations,
            &explicit.preparation_issues,
            &explicit.result.result,
        ),
        (
            automatic.candidate_pairs,
            automatic.signature_preparations,
            &automatic.preparation_issues,
            &automatic.result.result,
        ),
    ] {
        assert_eq!(candidates, 3);
        assert_eq!(preparations, 9);
        assert!(errors.is_empty());
        assert_eq!(report.comparisons.equal, vec![(0, 1), (3, 4), (6, 7)]);
        assert_eq!(
            report.grouping.groups,
            vec![vec![0, 1], vec![2], vec![3, 4], vec![5], vec![6, 7], vec![8]]
        );
        assert!(report.comparisons.issues.is_empty());
        assert!(report.source_issues.is_empty());
    }
    assert_eq!(budget.used(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // One labelled fixture covers both recursive routes, two budgets and retry.
fn indexed_recursive_memory_refusals_are_attributed_and_retry_recovers_all_edges() {
    use rrrah_dedup::container::{
        PresentationPolicy, scan_indexed_presentation_roots, scan_indexed_presentation_roots_auto,
    };
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
    let dir = tempfile::tempdir().unwrap();
    for (source, destination) in [
        ("base.gif", "a.gif"),
        ("split.apng", "b.apng"),
        ("zero-prefix.apng", "c.apng"),
    ] {
        std::fs::copy(fixtures.join(source), dir.path().join(destination)).unwrap();
    }
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 10,
        },
    };
    let roots = [dir.path().into()];
    let traversal = rrrah_dedup::exact::Options::default();
    let mode = |path: &std::path::Path| {
        Presentation::Animation(if path.extension().unwrap() == "gif" {
            AnimationKind::Gif
        } else {
            AnimationKind::Apng
        })
    };
    for bytes in [0, 1] {
        let budget = MemoryBudget::new(bytes);
        let explicit =
            scan_indexed_presentation_roots(&roots, &traversal, mode, policy, &budget, || false).unwrap();
        let automatic =
            scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
        assert!(automatic.result.detection_issues.is_empty());
        for (candidates, preparation, report) in [
            (
                explicit.candidate_pairs,
                &explicit.preparation_issues,
                &explicit.result.result,
            ),
            (
                automatic.candidate_pairs,
                &automatic.preparation_issues,
                &automatic.result.result,
            ),
        ] {
            assert_eq!(candidates, 3);
            assert_eq!(
                preparation.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                vec![0, 1, 2]
            );
            assert!(report.comparisons.equal.is_empty());
            assert!(report.comparisons.different.is_empty());
            assert_eq!(
                report
                    .comparisons
                    .issues
                    .iter()
                    .map(|issue| (issue.left, issue.right))
                    .collect::<Vec<_>>(),
                vec![(0, 1), (0, 2), (1, 2)]
            );
            for issue in &report.comparisons.issues {
                assert!(
                    matches!(
                        &issue.error,
                        ContainerError::Animation(
                            rrrah_dedup::animated::AnimationError::Memory(_)
                                | rrrah_dedup::animated::AnimationError::Decode(
                                    rrrah_dedup::decode::FileError::Decode(
                                        rrrah_decode::RasterDecodeError::Source(
                                            rrrah_decode::DecodeError::Memory(_)
                                        )
                                    )
                                )
                        )
                    ),
                    "unexpected issue: {:?}",
                    issue.error
                );
            }
            // These are inconclusive singleton partitions, with explicit issues.
            assert_eq!(report.grouping.groups, vec![vec![0], vec![1], vec![2]]);
        }
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let retry =
        scan_indexed_presentation_roots_auto(&roots, &traversal, 100, policy, &budget, || false).unwrap();
    assert!(retry.preparation_issues.is_empty());
    assert!(retry.result.result.comparisons.issues.is_empty());
    assert_eq!(
        retry.result.result.comparisons.equal,
        vec![(0, 1), (0, 2), (1, 2)]
    );
    assert_eq!(retry.result.result.grouping.groups, vec![vec![0, 1, 2]]);
    assert_eq!(budget.used(), 0);
}
