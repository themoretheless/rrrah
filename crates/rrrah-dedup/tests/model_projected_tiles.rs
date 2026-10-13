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
use rrrah_dedup::model_projected_tiles::{
    ProjectedTileAttempt, ProjectedTileError, ProjectedTileWorkLimits,
    search_projected_model_tiles_files_with,
};
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

#[test]
fn hdr_regions_recover_after_whole_miss_with_original_domain_mapping_and_shared_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    let p = policy();
    let calls = Cell::new(0);
    let result = search_projected_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ProjectedTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, a, b, budget, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 7 {
                assert_eq!(a.original, [4, 4]);
                assert_eq!(b.original, [4, 4]);
                region_proposals(budget)
            } else {
                empty(budget)
            }
        },
        || false,
    )
    .unwrap();
    assert!(result.supported());
    assert_eq!(result.attempts().len(), 7);
    assert_eq!(result.prepared_inputs(), [2, 5]);
    let attempt = result.attempts().last().unwrap();
    assert_eq!(attempt.rectangles, [[0, 0, 4, 4]; 2]);
    assert_eq!(attempt.evidence.original_dimensions, [[8, 8]; 2]);
    assert_eq!(
        attempt.evidence.transform.unwrap().matrix,
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
    );
    assert_eq!(attempt.evidence.pixels.unwrap().forward.matched_pixels, 64);
    assert_eq!(result.pixel_work(), 128);
    let credit = budget.used();
    assert!(credit > 0);
    let other = result.clone();
    drop(result);
    assert_eq!(budget.used(), credit);
    drop(other);
    assert_eq!(budget.used(), 0);
}

#[test]
fn all_25_equal_hdr_projections_are_rejected_by_originals() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    std::fs::write(&r[0].path, pfm(100.)).unwrap();
    std::fs::write(&r[1].path, pfm(110.)).unwrap();
    let p = policy();
    let budget = MemoryBudget::new(65536);
    let calls = Cell::new(0);
    let evidence = search_projected_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ProjectedTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, a, b, budget, _| {
            calls.set(calls.get() + 1);
            assert_eq!(&*a.pixels, &*b.pixels);
            proposals(budget)
        },
        || false,
    )
    .unwrap();
    assert!(!evidence.supported());
    assert_eq!(calls.get(), 25);
    assert_eq!(evidence.attempts().len(), 25);
    assert_eq!(evidence.prepared_inputs(), [5, 5]);
    assert!(evidence.attempts().all(|a| !a.evidence.supported));
    assert!(evidence.attempts().any(|a| a.evidence.pixels.is_some()));
    assert!(evidence.pixel_work() <= p.pixels.max_source_pixels * 25);
    drop(evidence);
    assert_eq!(budget.used(), 0);
}

#[test]
fn alpha_copies_support_and_equal_black_projections_with_different_alpha_exhaust() {
    let dir = tempfile::tempdir().unwrap();
    let (_, w) = fixture(dir.path());
    let r: [DecodeRequest; 2] =
        std::array::from_fn(|i| DecodeRequest::new(dir.path().join(format!("alpha-{i}.qoi"))));
    let write = |alpha: u8, path: &Path| {
        let mut b = b"qoif".to_vec();
        b.extend(8u32.to_be_bytes());
        b.extend(8u32.to_be_bytes());
        b.extend([4, 1]);
        for _ in 0..64 {
            b.extend([0xff, 0, 0, 0, alpha]);
        }
        b.extend([0, 0, 0, 0, 0, 0, 0, 1]);
        std::fs::write(path, b).unwrap();
    };
    let p = policy();
    let budget = MemoryBudget::new(65536);
    for other in [64, 255] {
        write(64, &r[0].path);
        write(other, &r[1].path);
        let e = search_projected_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ProjectedTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, a, b, budget, _| {
                assert_eq!(&*a.pixels, &*b.pixels);
                proposals(budget)
            },
            || false,
        )
        .unwrap();
        assert_eq!(e.supported(), other == 64);
        assert_eq!(e.attempts().len(), if other == 64 { 1 } else { 25 });
        drop(e);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn all_seven_work_caps_and_ledger_refuse_before_missing_io_and_cancel_wins() {
    let r = DecodeRequest::new("/missing/projected-tiles.pfm");
    let w = Path::new("/missing/weights");
    let p = policy();
    let required = ProjectedTileWorkLimits::for_model(p).unwrap();
    for field in 0..7 {
        let mut work = required;
        match field {
            0 => work.model_input_reads -= 1,
            1 => work.proposal_ranking_checks -= 1,
            2 => work.proposal_separation_checks -= 1,
            3 => work.geometry_hypotheses -= 1,
            4 => work.original_pixel_work -= 1,
            5 => work.identity_input_checks -= 1,
            _ => work.identity_point_checks -= 1,
        }
        let budget = MemoryBudget::new(65536);
        let e = search_projected_model_tiles_files_with(
            &r,
            &r,
            w,
            p,
            work,
            &budget,
            |_, _, _, _, _| panic!("provider"),
            || false,
        );
        assert!(matches!(e, Err(ProjectedTileError::Work)));
        assert_eq!(budget.peak(), 0);
        let e = search_projected_model_tiles_files_with(
            &r,
            &r,
            w,
            p,
            work,
            &budget,
            |_, _, _, _, _| panic!("provider"),
            || true,
        );
        assert!(matches!(e, Err(ProjectedTileError::Cancelled)));
    }
    let mut overflow = p;
    overflow.input.max_pixel_reads = u64::MAX;
    assert!(matches!(
        ProjectedTileWorkLimits::for_model(overflow),
        Err(ProjectedTileError::Work)
    ));
    let budget = MemoryBudget::new((25 * std::mem::size_of::<Option<ProjectedTileAttempt>>() - 1) as u64);
    let e = search_projected_model_tiles_files_with(
        &r,
        &r,
        w,
        p,
        required,
        &budget,
        |_, _, _, _, _| panic!("provider"),
        || false,
    );
    assert!(matches!(e, Err(ProjectedTileError::Buffer(_))));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn source_weights_and_provider_failures_after_a_miss_remain_terminal_and_release() {
    for mode in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let (r, w) = fixture(dir.path());
        let p = policy();
        let budget = MemoryBudget::new(65536);
        let calls = Cell::new(0);
        let e = search_projected_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ProjectedTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, budget, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    if mode == 3 {
                        return Err("provider failed".into());
                    }
                    let target = if mode == 2 { &w } else { &r[mode].path };
                    let mut b = std::fs::read(target).unwrap();
                    b.push(0);
                    std::fs::write(target, b).unwrap();
                }
                empty(budget)
            },
            || false,
        );
        assert!(matches!(e, Err(ProjectedTileError::Comparison(_))), "{e:?}");
        assert_eq!(calls.get(), 2);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn last_caller_source_and_weight_mutations_cannot_publish_a_negative_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let p = policy();
    let budget = MemoryBudget::new(65536);
    let count = Cell::new(0usize);
    drop(
        search_projected_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ProjectedTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, b, _| empty(b),
            || {
                count.set(count.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let last = count.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let bytes = std::fs::read(path).unwrap();
        count.set(0);
        let result = search_projected_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ProjectedTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, b, _| empty(b),
            || {
                count.set(count.get() + 1);
                if count.get() == last {
                    let mut b = bytes.clone();
                    b.push(0);
                    std::fs::write(path, b).unwrap();
                }
                false
            },
        );
        assert!(
            matches!(result, Err(ProjectedTileError::Comparison(_))),
            "{result:?}"
        );
        assert_eq!(count.get(), last);
        assert_eq!(budget.used(), 0);
        std::fs::write(path, bytes).unwrap();
    }
}

#[test]
fn global_and_both_owner_generations_cancel_first_middle_and_final_checkpoints() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let p = policy();
    let budget = MemoryBudget::new(65536);
    let count = Cell::new(0usize);
    let work = ProjectedTileWorkLimits::for_model(p).unwrap();
    drop(
        search_projected_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
            || {
                count.set(count.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let total = count.get();
    for stop in [1, total / 2, total] {
        for owner in 0..3 {
            let mut r = r.clone();
            let generation = Arc::new(AtomicU64::new(1));
            if owner > 0 {
                r[owner - 1].cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
            }
            count.set(0);
            let result = search_projected_model_tiles_files_with(
                &r[0],
                &r[1],
                &w,
                p,
                work,
                &budget,
                |_, _, _, b, _| empty(b),
                || {
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        if owner == 0 {
                            return true;
                        }
                        generation.store(2, Ordering::Release);
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(ProjectedTileError::Cancelled)),
                "owner={owner} stop={stop}: {result:?}"
            );
            assert_eq!(count.get(), stop);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn different_nonzero_crop_origins_map_proposals_to_full_original_coordinates() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    for (index, request) in r.iter().enumerate() {
        let mut bytes = b"PF\n8 8\n-1.0\n".to_vec();
        for y in (0..8).rev() {
            for x in 0..8 {
                let value = if index == 0 {
                    4. + x as f32 + 8. * y as f32
                } else if x < 4 && y < 4 {
                    4. + (x + 4) as f32 + 8. * (y + 4) as f32
                } else {
                    100. + x as f32 + 8. * y as f32
                };
                for _ in 0..3 {
                    bytes.extend(value.to_le_bytes());
                }
            }
        }
        std::fs::write(&request.path, bytes).unwrap();
    }
    let mut p = policy();
    p.minimum_coverage = 0.1;
    let budget = MemoryBudget::new(65536);
    let calls = Cell::new(0);
    let e = search_projected_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ProjectedTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 22 {
                region_proposals(b)
            } else {
                empty(b)
            }
        },
        || false,
    )
    .unwrap();
    assert!(e.supported());
    assert_eq!(e.attempts().len(), 22);
    let a = e.attempts().last().unwrap();
    assert_eq!(a.rectangles, [[4, 4, 4, 4], [0, 0, 4, 4]]);
    let mapped = a.evidence.transform.unwrap().apply([5., 5.]).unwrap();
    assert!(
        (mapped[0] - 1.).abs() < 1e-7 && (mapped[1] - 1.).abs() < 1e-7,
        "{mapped:?}"
    );
    assert_eq!(a.evidence.pixel_attempts, 1);
    drop(e);
    assert_eq!(budget.used(), 0);
}
