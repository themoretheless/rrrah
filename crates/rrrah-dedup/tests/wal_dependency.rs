#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    container::{ContainerError, Presentation, same_presentation},
};

#[test]
fn selected_presentation_rejects_external_palette_mutation_and_recovers() {
    let folder = tempfile::tempdir().unwrap();
    let mut requests = Vec::new();
    for id in 0..2 {
        let root = folder.path().join(format!("game-{id}"));
        std::fs::create_dir_all(root.join("textures")).unwrap();
        std::fs::create_dir(root.join("pics")).unwrap();
        let path = root.join("textures/source.wal");
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
        )
        .unwrap();
        std::fs::write(
            root.join("pics/colormap.pcx"),
            include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
        )
        .unwrap();
        requests.push(DecodeRequest::new(path));
    }
    let palette = folder.path().join("game-0/pics/colormap.pcx");
    let original = std::fs::read(&palette).unwrap();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 1,
        max_pixels: 30_000,
        max_file_bytes: 1024 * 1024,
    };
    let compare = |cancel| {
        same_presentation(
            (&requests[0], Presentation::SelectedFrame),
            (&requests[1], Presentation::SelectedFrame),
            limits,
            &budget,
            cancel,
        )
    };
    let changed = std::cell::Cell::new(false);
    let cancel = || {
        if budget.used() > 0 && !changed.replace(true) {
            let mut bytes = original.clone();
            let start = bytes.len() - 768;
            for value in &mut bytes[start..] {
                *value = 255 - *value;
            }
            std::fs::write(&palette, bytes).unwrap();
        }
        false
    };
    let result = compare(&cancel as &dyn Fn() -> bool);
    assert!(
        changed.get(),
        "mutation must run while decoded resources are retained"
    );
    match result {
        Err(ContainerError::Source(rrrah_dedup::exact::SnapshotError::Changed)) => {}
        Err(ContainerError::File(rrrah_dedup::decode::FileError::Prepared(error))) => {
            assert!(matches!(
                *error,
                rrrah_dedup::decode::CachedError::Source(rrrah_dedup::exact::SnapshotError::Changed)
            ));
        }
        other => panic!("changed dependency must invalidate comparison: {other:?}"),
    }
    assert_eq!(budget.used(), 0);
    std::fs::write(&palette, &original).unwrap();
    assert!(compare(&(|| false) as &dyn Fn() -> bool).unwrap());
    assert_eq!(budget.used(), 0);
}

#[test]
fn presentation_groups_remove_external_palette_stale_edges_and_retry() {
    use rrrah_dedup::container::{
        PresentationPolicy, scan_indexed_presentation_groups, scan_presentation_groups,
    };
    let folder = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for id in 1..=3 {
        let root = folder.path().join(format!("game-{id}"));
        std::fs::create_dir_all(root.join("textures")).unwrap();
        std::fs::create_dir(root.join("pics")).unwrap();
        let path = root.join("textures/source.wal");
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
        )
        .unwrap();
        std::fs::write(
            root.join("pics/colormap.pcx"),
            include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
        )
        .unwrap();
        files.push((id, DecodeRequest::new(path), Presentation::SelectedFrame));
    }
    let palette = folder.path().join("game-1/pics/colormap.pcx");
    let original = std::fs::read(&palette).unwrap();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let policy = PresentationPolicy {
        max_files: 3,
        max_pairs: 3,
        limits: AnimationBudget {
            max_frames: 1,
            max_pixels: 30_000,
            max_file_bytes: 1024 * 1024,
        },
        grouping: rrrah_dedup::groups::GroupBudget {
            max_entries: 3,
            max_pairs: 3,
            max_pair_checks: 100,
        },
    };
    for (indexed, removed, replaced) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (true, true, false),
        (false, false, true),
        (true, false, true),
    ] {
        std::fs::write(&palette, &original).unwrap();
        let active = std::cell::Cell::new(false);
        let changed = std::cell::Cell::new(false);
        let cancel = || {
            if budget.used() > 0 {
                active.set(true);
            } else if active.get() && !changed.replace(true) {
                if removed {
                    std::fs::remove_file(&palette).unwrap();
                } else if replaced {
                    let replacement = palette.with_extension("replacement.pcx");
                    std::fs::write(&replacement, &original).unwrap();
                    std::fs::rename(replacement, &palette).unwrap();
                } else {
                    let mut bytes = original.clone();
                    let start = bytes.len() - 768;
                    for value in &mut bytes[start..] {
                        *value = 255 - *value;
                    }
                    std::fs::write(&palette, bytes).unwrap();
                }
            }
            false
        };
        let run = |cancel: &dyn Fn() -> bool| {
            if indexed {
                scan_indexed_presentation_groups(files.clone(), policy, &budget, cancel)
                    .unwrap()
                    .result
            } else {
                scan_presentation_groups(files.clone(), policy, &budget, cancel).unwrap()
            }
        };
        let result = run(&cancel);
        assert!(changed.get(), "indexed={indexed}");
        assert_eq!(result.comparisons.equal, [(2, 3)], "indexed={indexed}");
        assert!(result.comparisons.different.is_empty(), "indexed={indexed}");
        assert_eq!(result.grouping.groups, [vec![1], vec![2, 3]], "indexed={indexed}");
        assert!(
            result
                .source_issues
                .iter()
                .any(|(id, error)| *id == 1 && if removed {
                    matches!(error, rrrah_dedup::exact::SnapshotError::Io(io) if io.kind() == std::io::ErrorKind::NotFound)
                } else {
                    matches!(error, rrrah_dedup::exact::SnapshotError::Changed)
                })
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(&palette, &original).unwrap();
        let retry = run(&|| false);
        assert_eq!(
            retry.comparisons.equal,
            [(1, 2), (1, 3), (2, 3)],
            "indexed={indexed}"
        );
        assert_eq!(retry.grouping.groups, [vec![1, 2, 3]], "indexed={indexed}");
        assert!(retry.source_issues.is_empty());
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn wal_fingerprint_one_shot_cancellation_never_admits_partial_cache_and_retries() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir(folder.path().join("textures")).unwrap();
    std::fs::create_dir(folder.path().join("pics")).unwrap();
    let path = folder.path().join("textures/source.wal");
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
    )
    .unwrap();
    std::fs::write(
        folder.path().join("pics/colormap.pcx"),
        include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
    )
    .unwrap();
    let request = DecodeRequest::new(path);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [81; 32],
        max_file_bytes: 1024 * 1024,
        max_pixels: 30_000,
        max_frames: 1,
        max_cache_entries: 4,
    };
    for warm in [false, true] {
        let mut baseline = FingerprintCache::default();
        if warm {
            fingerprint_file(&request, policy, &budget, &mut baseline, || false).unwrap();
        }
        let calls = std::cell::Cell::new(0);
        fingerprint_file(&request, policy, &budget, &mut baseline, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert!(calls.get() > 0);
        eprintln!("WAL cancellation checkpoints: warm={warm}, count={}", calls.get());
        for checkpoint in 1..=calls.get() {
            let mut cache = FingerprintCache::default();
            if warm {
                fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
            }
            let seen = std::cell::Cell::new(0);
            let cancelled = fingerprint_file(&request, policy, &budget, &mut cache, || {
                seen.set(seen.get() + 1);
                seen.get() == checkpoint
            });
            assert!(
                seen.get() >= checkpoint,
                "checkpoint must be reached: warm={warm}, checkpoint={checkpoint}"
            );
            let error = cancelled.expect_err("one-shot cancellation must abort processing");
            assert!(
                error.to_string().to_ascii_lowercase().contains("cancel"),
                "expected cancellation, got {error}: warm={warm}, checkpoint={checkpoint}"
            );
            assert_eq!(budget.used(), 0);
            let retry = fingerprint_file(&request, policy, &budget, &mut cache, || false).unwrap();
            assert_eq!(
                retry.cache_hit, warm,
                "cancelled miss must not populate cache: checkpoint={checkpoint}"
            );
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn wal_external_palette_obeys_source_byte_limit_even_with_warm_cache() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{CachedError, FingerprintPolicy, fingerprint_file},
    };
    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir(folder.path().join("textures")).unwrap();
    std::fs::create_dir(folder.path().join("pics")).unwrap();
    let path = folder.path().join("textures/source.wal");
    let wal = include_bytes!("../../../tests/fixtures/raster/palette-grid.wal");
    let pcx = include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx");
    assert!(pcx.len() > wal.len());
    std::fs::write(&path, wal).unwrap();
    std::fs::write(folder.path().join("pics/colormap.pcx"), pcx).unwrap();
    let request = DecodeRequest::new(path);
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let policy = FingerprintPolicy {
        recipe: [82; 32],
        max_file_bytes: 1024 * 1024,
        max_pixels: 30_000,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let mut cache = FingerprintCache::default();
    assert!(
        !fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    let limited = FingerprintPolicy {
        max_file_bytes: wal.len() as u64,
        ..policy
    };
    assert!(matches!(
        fingerprint_file(&request, limited, &budget, &mut cache, || false),
        Err(CachedError::Source(rrrah_dedup::exact::SnapshotError::Policy))
    ));
    assert_eq!(budget.used(), 0);
    assert!(
        fingerprint_file(&request, policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert_eq!(budget.used(), 0);
}
