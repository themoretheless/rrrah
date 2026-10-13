#![cfg(feature = "decode")]
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    model_compare::ProposalProviderError,
    model_input::ModelInputPolicy,
    model_projected_compare::ProjectedModelPolicy,
    model_projection::ModelInputProjection,
    proposals::{ProposalPolicy, ScoredProposal},
    warp::WarpPolicy,
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
};
fn policy() -> ProjectedModelPolicy {
    ProjectedModelPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 64,
            max_file_bytes: 4096,
        },
        input: ModelInputPolicy {
            max_side: 8,
            max_source_pixels: 64,
            max_output_pixels: 64,
            max_pixel_reads: 64,
        },
        projection: ModelInputProjection::CompositeReinhardV1 {
            exposure: 1.,
            background: [0.; 3],
        },
        proposals: ProposalPolicy {
            max_proposals: 6,
            max_ranking_checks: 1000,
            max_separation_checks: 1000,
            minimum_separation: 1.,
        },
        geometry: GeometryPolicy {
            tolerance: 0.001,
            min_inliers: 6,
            max_points: 6,
            max_hypotheses: 64,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 64,
            seed: 173,
        },
        pixels: WarpPolicy {
            tolerance: 0.00001,
            max_source_pixels: 256,
        },
        minimum_coverage: 1.,
        minimum_agreement: 1.,
        max_model_bytes: 1024,
    }
}
fn pfm(value: f32) -> Vec<u8> {
    let mut b = b"PF\n8 8\n-1.0\n".to_vec();
    for _ in 0..64 * 3 {
        b.extend(value.to_le_bytes());
    }
    b
}
fn fixture(dir: &Path) -> ([DecodeRequest; 2], PathBuf) {
    let requests = std::array::from_fn(|i| {
        let path = dir.join(format!("{i}.pfm"));
        std::fs::write(&path, pfm(4.)).unwrap();
        let mut r = DecodeRequest::new(path);
        r.assume_untagged_linear_srgb = true;
        r
    });
    let weights = dir.join("weights");
    std::fs::write(&weights, b"fixture weights").unwrap();
    (requests, weights)
}
fn proposals(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut b = budget.try_buffer(
        6,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in b.iter_mut().enumerate() {
        let q = [1. + 2. * (i % 3) as f64, 1. + 4. * (i / 3) as f64];
        p.source = q;
        p.target = q;
    }
    Ok(b.freeze())
}
fn region_proposals(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut b = budget.try_buffer(
        6,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in b.iter_mut().enumerate() {
        let q = [[0.75, 3.25, 6.25][i % 3], [0.75, 6.25][i / 3]];
        p.source = q;
        p.target = q;
    }
    Ok(b.freeze())
}
use rrrah_dedup::model_projected_tiles::{ProjectedTileError, ProjectedTileWorkLimits};
fn empty(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    Ok(budget
        .try_buffer(
            0,
            ScoredProposal {
                source: [0.; 2],
                target: [0.; 2],
                confidence: 1.,
            },
        )?
        .freeze())
}

use rrrah_dedup::projected_collection::{
    ProjectedCollectionError, ProjectedCollectionPolicy, scan_projected_model_collection_with,
};
fn collection_policy() -> ProjectedCollectionPolicy {
    let model = policy();
    let pair_work = ProjectedTileWorkLimits::for_model(model).unwrap();
    ProjectedCollectionPolicy {
        model,
        pair_work,
        max_files: 3,
        max_pairs: 3,
        work: pair_work.for_pairs(3).unwrap(),
    }
}
fn collection_fixture(dir: &Path) -> (Vec<(u64, DecodeRequest)>, PathBuf) {
    let (r, w) = fixture(dir);
    let third = dir.join("third.pfm");
    std::fs::write(&third, pfm(4.)).unwrap();
    let mut request = DecodeRequest::new(third);
    request.assume_untagged_linear_srgb = true;
    (vec![(9, request), (3, r[1].clone()), (1, r[0].clone())], w)
}
#[test]
fn all_pairs_recover_regions_in_id_order_and_keep_managed_ledgers() {
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    let budget = MemoryBudget::new(262144);
    let calls = Cell::new(0);
    let report = scan_projected_model_collection_with(
        files,
        &w,
        collection_policy(),
        &budget,
        |_, _, _, b, _| {
            let n = calls.get() + 1;
            calls.set(n);
            if n % 7 == 0 { region_proposals(b) } else { empty(b) }
        },
        || false,
    )
    .unwrap();
    assert_eq!(report.file_ids(), &[1, 3, 9]);
    assert_eq!(
        report
            .pairs()
            .iter()
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>(),
        [(1, 3), (1, 9), (3, 9)]
    );
    assert_eq!(calls.get(), 21);
    for p in report.pairs() {
        assert!(p.evidence.supported());
        assert_eq!(p.evidence.attempts().len(), 7);
        assert_eq!(p.evidence.prepared_inputs(), [2, 5]);
    }
    let ledger = report.pairs()[0].evidence.clone();
    drop(report);
    assert!(budget.used() > 0);
    drop(ledger);
    assert_eq!(budget.used(), 0);
}
#[test]
fn different_hdr_originals_remain_negative_in_complete_collection() {
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    for (id, r) in &files {
        std::fs::write(&r.path, pfm(if *id == 9 { 110. } else { 100. })).unwrap();
    }
    let budget = MemoryBudget::new(262144);
    let report = scan_projected_model_collection_with(
        files,
        &w,
        collection_policy(),
        &budget,
        |_, _, _, b, _| region_proposals(b),
        || false,
    )
    .unwrap();
    assert_eq!(report.pairs().len(), 3);
    assert!(report.pairs()[0].evidence.supported());
    for pair in &report.pairs()[1..] {
        assert!(!pair.evidence.supported());
        assert_eq!(pair.evidence.attempts().len(), 25);
    }
    drop(report);
    assert_eq!(budget.used(), 0);
}
#[test]
fn cumulative_work_and_cancellation_refuse_before_iterator_consumption() {
    for field in 0..7 {
        let mut p = collection_policy();
        let v = match field {
            0 => &mut p.work.model_input_reads,
            1 => &mut p.work.proposal_ranking_checks,
            2 => &mut p.work.proposal_separation_checks,
            3 => &mut p.work.geometry_hypotheses,
            4 => &mut p.work.original_pixel_work,
            5 => &mut p.work.identity_input_checks,
            _ => &mut p.work.identity_point_checks,
        };
        *v -= 1;
        let budget = MemoryBudget::new(262144);
        for cancelled in [false, true] {
            let result = scan_projected_model_collection_with(
                std::iter::from_fn(|| panic!("consumed")),
                Path::new("missing"),
                p,
                &budget,
                |_, _, _, b, _| proposals(b),
                || cancelled,
            );
            if cancelled {
                assert!(matches!(result, Err(ProjectedCollectionError::Cancelled)));
            } else {
                assert!(matches!(result, Err(ProjectedCollectionError::Work)));
            }
            assert_eq!(budget.peak(), 0);
        }
    }
    let mut work = collection_policy().pair_work;
    work.original_pixel_work = u64::MAX;
    assert!(matches!(work.for_pairs(2), Err(ProjectedTileError::Work)));
}
#[test]
fn source_weights_and_provider_failures_discard_prior_supported_pairs() {
    for kind in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let (files, w) = collection_fixture(dir.path());
        let third = files[0].1.path.clone();
        let budget = MemoryBudget::new(262144);
        let calls = Cell::new(0);
        let result = scan_projected_model_collection_with(
            files,
            &w,
            collection_policy(),
            &budget,
            |_, _, _, b, _| {
                let n = calls.get() + 1;
                calls.set(n);
                if kind == 0 && n == 1 {
                    std::fs::write(&third, pfm(8.)).unwrap();
                }
                if kind == 1 && n == 2 {
                    std::fs::write(&w, b"mutated weights").unwrap();
                }
                if kind == 2 && n == 2 {
                    return Err("test provider".into());
                }
                region_proposals(b)
            },
            || false,
        );
        match kind {
            0 => assert!(matches!(
                result,
                Err(ProjectedCollectionError::Source { id: 9, .. })
            )),
            1 => assert!(matches!(
                result,
                Err(ProjectedCollectionError::Pair {
                    left: 1,
                    right: 9,
                    ..
                })
            )),
            _ => assert!(matches!(
                result,
                Err(ProjectedCollectionError::Pair {
                    left: 1,
                    right: 9,
                    error: ProjectedTileError::Comparison(_)
                })
            )),
        }
        assert_eq!(calls.get(), if kind == 0 { 1 } else { 2 });
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn last_collection_checkpoint_cannot_publish_mutated_inputs_or_weights() {
    for target in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let (files, w) = collection_fixture(dir.path());
        let paths = files.iter().map(|(_, r)| r.path.clone()).collect::<Vec<_>>();
        let calls = Cell::new(0);
        let budget = MemoryBudget::new(262144);
        let baseline = scan_projected_model_collection_with(
            files.clone(),
            &w,
            collection_policy(),
            &budget,
            |_, _, _, b, _| region_proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
        let last = calls.get();
        drop(baseline);
        assert_eq!(budget.used(), 0);
        calls.set(0);
        let result = scan_projected_model_collection_with(
            files,
            &w,
            collection_policy(),
            &budget,
            |_, _, _, b, _| region_proposals(b),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == last {
                    if target < 3 {
                        std::fs::write(&paths[target], pfm(8.)).unwrap();
                    } else {
                        std::fs::write(&w, b"mutated").unwrap();
                    }
                }
                false
            },
        );
        assert_eq!(calls.get(), last);
        if target < 3 {
            assert!(matches!(result, Err(ProjectedCollectionError::Source { .. })));
        } else {
            assert!(matches!(result, Err(ProjectedCollectionError::ModelSource(_))));
        }
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn global_and_every_selected_owner_cancel_at_first_middle_and_final_checkpoint() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let (mut files, w) = collection_fixture(dir.path());
        let generation = Arc::new(AtomicU64::new(1));
        if owner < 3 {
            files[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        }
        let budget = MemoryBudget::new(262144);
        let calls = Cell::new(0);
        drop(
            scan_projected_model_collection_with(
                files.clone(),
                &w,
                collection_policy(),
                &budget,
                |_, _, _, b, _| region_proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap(),
        );
        let last = calls.get();
        for trigger in [if owner < 3 { last / 3 } else { 1 }, last / 2, last] {
            calls.set(0);
            generation.store(1, Ordering::Release);
            let result = scan_projected_model_collection_with(
                files.clone(),
                &w,
                collection_policy(),
                &budget,
                |_, _, _, b, _| region_proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == trigger {
                        if owner < 3 {
                            generation.store(2, Ordering::Release);
                        }
                        return owner == 3;
                    }
                    if calls.get() > trigger {
                        generation.store(1, Ordering::Release);
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(ProjectedCollectionError::Cancelled)),
                "owner={owner} trigger={trigger}: {result:?}"
            );
            assert_eq!(calls.get(), trigger);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn collection_admission_never_truncates_and_empty_batch_still_guards_weights() {
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    let budget = MemoryBudget::new(262144);
    let mut p = collection_policy();
    p.max_pairs = 2;
    p.work = p.pair_work.for_pairs(2).unwrap();
    let result = scan_projected_model_collection_with(
        files.clone(),
        &w,
        p,
        &budget,
        |_, _, _, _, _| panic!("inference"),
        || false,
    );
    assert!(matches!(result, Err(ProjectedCollectionError::Pairs)));
    assert_eq!(budget.used(), 0);
    let mut duplicated = files.clone();
    duplicated[0].0 = duplicated[1].0;
    assert!(matches!(
        scan_projected_model_collection_with(
            duplicated,
            &w,
            collection_policy(),
            &budget,
            |_, _, _, _, _| panic!("inference"),
            || false
        ),
        Err(ProjectedCollectionError::DuplicateId(3))
    ));
    let mut p = collection_policy();
    p.max_files = 2;
    assert!(matches!(
        scan_projected_model_collection_with(
            files,
            &w,
            p,
            &budget,
            |_, _, _, _, _| panic!("inference"),
            || false
        ),
        Err(ProjectedCollectionError::Files)
    ));
    let empty = scan_projected_model_collection_with(
        [],
        &w,
        collection_policy(),
        &budget,
        |_, _, _, _, _| panic!("inference"),
        || false,
    )
    .unwrap();
    assert!(empty.file_ids().is_empty());
    assert!(empty.pairs().is_empty());
    drop(empty);
    assert!(matches!(
        scan_projected_model_collection_with(
            [],
            Path::new("missing weights"),
            collection_policy(),
            &budget,
            |_, _, _, _, _| panic!("inference"),
            || false
        ),
        Err(ProjectedCollectionError::ModelSource(_))
    ));
    assert_eq!(budget.used(), 0);
    let tiny = MemoryBudget::new(0);
    assert!(matches!(
        scan_projected_model_collection_with(
            std::iter::from_fn(|| panic!("consumed")),
            &w,
            collection_policy(),
            &tiny,
            |_, _, _, _, _| panic!("inference"),
            || false
        ),
        Err(ProjectedCollectionError::Memory(_))
    ));
    assert_eq!(tiny.peak(), 0);
}

#[test]
fn external_palette_mutation_or_removal_discards_prior_support_and_final_collection() {
    for remove in [false, true] {
        for final_checkpoint in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (mut files, weights) = collection_fixture(dir.path());
            std::fs::create_dir(dir.path().join("textures")).unwrap();
            std::fs::create_dir(dir.path().join("pics")).unwrap();
            let wal = dir.path().join("textures/third.wal");
            let palette = dir.path().join("pics/colormap.pcx");
            std::fs::write(
                &wal,
                include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
            )
            .unwrap();
            std::fs::write(
                &palette,
                include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
            )
            .unwrap();
            files[0].1 = DecodeRequest::new(wal);
            let mut policy = collection_policy();
            policy.model.decode.max_pixels = 256;
            policy.model.decode.max_file_bytes = 1024 * 1024;
            policy.model.input.max_source_pixels = 256;
            policy.model.input.max_pixel_reads = 1024;
            policy.model.pixels.max_source_pixels = 1024;
            policy.pair_work = ProjectedTileWorkLimits::for_model(policy.model).unwrap();
            policy.work = policy.pair_work.for_pairs(policy.max_pairs).unwrap();
            let budget = MemoryBudget::new(8 * 1024 * 1024);
            let calls = Cell::new(0usize);
            let providers = Cell::new(0usize);
            let run_provider = |b: &MemoryBudget| {
                let n = providers.get() + 1;
                providers.set(n);
                if n == 1 { region_proposals(b) } else { empty(b) }
            };
            let baseline = scan_projected_model_collection_with(
                files.clone(),
                &weights,
                policy,
                &budget,
                |_, _, _, b, _| run_provider(b),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap();
            assert!(baseline.pairs()[0].evidence.supported());
            assert_eq!(baseline.pairs().len(), 3);
            assert_eq!(providers.get(), 51);
            let last = calls.get();
            drop(baseline);
            assert_eq!(budget.used(), 0);
            calls.set(0);
            providers.set(0);
            let mutate = || {
                if remove {
                    std::fs::remove_file(&palette).unwrap();
                } else {
                    let mut bytes = std::fs::read(&palette).unwrap();
                    bytes.push(0);
                    std::fs::write(&palette, bytes).unwrap();
                }
            };
            let result = scan_projected_model_collection_with(
                files,
                &weights,
                policy,
                &budget,
                |_, _, _, b, _| {
                    let p = run_provider(b)?;
                    if !final_checkpoint && providers.get() == 1 {
                        mutate();
                    }
                    Ok(p)
                },
                || {
                    calls.set(calls.get() + 1);
                    if final_checkpoint && calls.get() == last {
                        mutate();
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(ProjectedCollectionError::Source { id: 9, .. })),
                "remove={remove} final={final_checkpoint}: {result:?}"
            );
            assert_eq!(providers.get(), if final_checkpoint { 51 } else { 1 });
            if final_checkpoint {
                assert_eq!(calls.get(), last);
            }
            assert_eq!(budget.used(), 0);
        }
    }
}
