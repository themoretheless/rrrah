#![cfg(feature = "decode")]
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::{
        ModelComparisonError, ModelComparisonPolicy, ProposalProviderError, compare_model_files_with,
    },
    model_input::ModelInputPolicy,
    proposals::{ProposalPolicy, ScoredProposal},
    rank_region::RankRegionPolicy,
};
use std::{
    cell::Cell,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
fn policy() -> ModelComparisonPolicy {
    ModelComparisonPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 4096,
            max_file_bytes: 65536,
        },
        input: ModelInputPolicy {
            max_side: 64,
            max_source_pixels: 4096,
            max_output_pixels: 4096,
            max_pixel_reads: 4096,
        },
        proposals: ProposalPolicy {
            max_proposals: 16,
            max_ranking_checks: 10000,
            max_separation_checks: 10000,
            minimum_separation: 2.,
        },
        geometry: GeometryPolicy {
            tolerance: 0.01,
            min_inliers: 10,
            max_points: 16,
            max_hypotheses: 64,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 64,
            seed: 173,
        },
        anchors: AnchorRankPolicy {
            local: LocalRankPolicy {
                rank: RankRegionPolicy {
                    radius: 2,
                    minimum_contrast: 0.001,
                    minimum_pairs: 100,
                    maximum_sites: 100000,
                    maximum_pixel_reads: 500000,
                },
                filter_radius: 0,
                minimum_witnesses: 10,
                maximum_points: 16,
                maximum_point_checks: 10000,
                minimum_point_separation: 2.,
                target_tolerance: 0.01,
                source_tolerance: 0.01,
                minimum_coverage: 0.3,
                minimum_agreement: 0.9,
            },
            window_radius: 1,
            maximum_selection_checks: 10000,
        },
        max_model_bytes: 1024,
    }
}
fn fixture(dir: &Path) -> ([DecodeRequest; 2], std::path::PathBuf) {
    let mut b = Vec::new();
    b.extend(b"BM");
    b.extend((54u32 + 64 * 64 * 3).to_le_bytes());
    b.extend([0; 4]);
    b.extend(54u32.to_le_bytes());
    b.extend(40u32.to_le_bytes());
    b.extend(64i32.to_le_bytes());
    b.extend(64i32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(24u16.to_le_bytes());
    b.extend([0; 24]);
    for y in 0..64u32 {
        for x in 0..64u32 {
            b.extend([(x * y / 16) as u8, (y * 3) as u8, (x * 3) as u8]);
        }
    }
    let requests = std::array::from_fn(|i| {
        let p = dir.join(format!("{i}.bmp"));
        std::fs::write(&p, &b).unwrap();
        DecodeRequest::new(p)
    });
    let weights = dir.join("weights.bin");
    std::fs::write(&weights, b"fixture provider only").unwrap();
    (requests, weights)
}
fn proposals(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut b = budget.try_buffer(
        10,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in b.iter_mut().enumerate() {
        let position = [8. + (i % 5) as f64 * 8., 8. + (i / 5) as f64 * 16.];
        p.source = position;
        p.target = position;
    }
    Ok(b.freeze())
}

fn region_proposals(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut points = budget.try_buffer(
        10,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in points.iter_mut().enumerate() {
        // The same original pixels occur at different crop-local coordinates.
        p.source = [4. + (i % 5) as f64 * 4., 12. + (i / 5) as f64 * 8.];
        p.target = [p.source[0] + 8., p.source[1] - 8.];
    }
    Ok(points.freeze())
}

#[test]
fn final_caller_checkpoint_cannot_publish_a_model_result_after_source_or_weight_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    drop(
        compare_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let last = calls.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let current = Cell::new(0);
        let e = compare_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                current.set(current.get() + 1);
                if current.get() == last {
                    let mut changed = original.clone();
                    changed.push(0);
                    std::fs::write(path, changed).unwrap();
                }
                false
            },
        );
        assert!(
            matches!(
                e,
                Err(ModelComparisonError::Left(_)
                    | ModelComparisonError::Right(_)
                    | ModelComparisonError::ModelSource(_))
            ),
            "{e:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
}

#[test]
fn shifted_crop_domains_confirm_the_same_original_pixels_without_a_translation_error() {
    use rrrah_dedup::model_compare::compare_model_regions_files_with;
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let e = compare_model_regions_files_with(
        &r[0],
        &r[1],
        &w,
        [16, 8, 32, 32],
        [8, 16, 32, 32],
        policy(),
        &budget,
        |_, a, b, budget, _| {
            assert_eq!(a.dimensions, [32, 32]);
            assert_eq!(a.original, [32, 32]);
            assert_eq!(b.dimensions, [32, 32]);
            region_proposals(budget)
        },
        || false,
    )
    .unwrap();
    assert!(e.supported(), "{e:?}");
    assert_eq!(e.geometry_inliers, 10);
    let h = e.transform.unwrap().matrix;
    for (y, row) in h.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            assert!((value - if x == y { 1. } else { 0. }).abs() < 1e-8);
        }
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn region_lifecycle_refuses_whole_source_mutation_and_both_request_cancellations() {
    use rrrah_dedup::model_compare::compare_model_regions_files_with;
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let result = compare_model_regions_files_with(
            &r[0],
            &r[1],
            &w,
            [16, 8, 32, 32],
            [8, 16, 32, 32],
            policy(),
            &budget,
            |_, _, _, b, _| {
                let mut changed = original.clone();
                changed.push(0);
                std::fs::write(path, changed)?;
                region_proposals(b)
            },
            || false,
        );
        assert!(
            matches!(
                result,
                Err(ModelComparisonError::Left(_)
                    | ModelComparisonError::Right(_)
                    | ModelComparisonError::ModelSource(_))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
    for owner in 0..2 {
        let mut requests = r.clone();
        let generation = Arc::new(AtomicU64::new(1));
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let result = compare_model_regions_files_with(
            &requests[0],
            &requests[1],
            &w,
            [16, 8, 32, 32],
            [8, 16, 32, 32],
            policy(),
            &budget,
            |_, _, _, b, cancel| {
                generation.store(2, Ordering::Release);
                assert!(cancel());
                generation.store(1, Ordering::Release);
                region_proposals(b)
            },
            || false,
        );
        assert!(matches!(result, Err(ModelComparisonError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn malformed_regions_refuse_before_weight_io_or_provider_and_global_cancel_wins() {
    use rrrah_dedup::model_compare::compare_model_regions_files_with;
    let r = DecodeRequest::new("missing");
    let budget = MemoryBudget::new(0);
    for (left, right) in [([0, 0, 0, 1], [0, 0, 1, 1]), ([0, 0, 1, 1], [u32::MAX, 0, 2, 1])] {
        assert!(matches!(
            compare_model_regions_files_with(
                &r,
                &r,
                Path::new("missing"),
                left,
                right,
                policy(),
                &budget,
                |_, _, _, _, _| panic!("provider must not run"),
                || false
            ),
            Err(ModelComparisonError::Left(_) | ModelComparisonError::Right(_))
        ));
        assert!(matches!(
            compare_model_regions_files_with(
                &r,
                &r,
                Path::new("missing"),
                left,
                right,
                policy(),
                &budget,
                |_, _, _, _, _| panic!("provider must not run"),
                || true
            ),
            Err(ModelComparisonError::Cancelled)
        ));
    }
    assert_eq!((budget.used(), budget.peak()), (0, 0));
}
#[test]
fn supplied_provider_supports_original_pixels_empty_is_explicit_and_errors_do_not_become_misses() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let e = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |path, a, b, budget, _| {
            assert_eq!(path, w);
            assert_eq!(a.original, b.original);
            proposals(budget)
        },
        || false,
    )
    .unwrap();
    assert!(e.supported());
    assert_eq!(
        (e.raw_proposals, e.selected_points, e.geometry_inliers),
        (10, 10, 10)
    );
    assert_eq!(e.pixels.unwrap().forward_anchors, 10);
    assert_eq!(budget.used(), 0);
    let e = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, budget, _| {
            Ok(budget
                .try_buffer(
                    0,
                    ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 0.,
                    },
                )?
                .freeze())
        },
        || false,
    )
    .unwrap();
    assert!(!e.supported());
    assert!(e.transform.is_none() && e.pixels.is_none());
    assert_eq!(e.hypotheses, 0);
    assert_eq!(budget.used(), 0);
    let error = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, _, _| Err(std::io::Error::other("fixture inference failure").into()),
        || false,
    );
    assert!(matches!(error, Err(ModelComparisonError::Provider(_))));
    assert_eq!(budget.used(), 0);
}
#[test]
fn either_source_or_model_mutation_during_provider_refuses_and_restored_retry_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let e = compare_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, budget, _| {
                let mut changed = original.clone();
                changed.push(0);
                std::fs::write(path, changed)?;
                proposals(budget)
            },
            || false,
        );
        assert!(
            matches!(
                e,
                Err(ModelComparisonError::Left(_)
                    | ModelComparisonError::Right(_)
                    | ModelComparisonError::ModelSource(_))
            ),
            "{e:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
        assert!(
            compare_model_files_with(
                &r[0],
                &r[1],
                &w,
                policy(),
                &budget,
                |_, _, _, b, _| proposals(b),
                || false
            )
            .unwrap()
            .supported()
        );
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn both_generations_and_ignored_provider_cancellation_are_sticky() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for owner in 0..2 {
        let generation = Arc::new(AtomicU64::new(1));
        let mut requests = r.clone();
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let e = compare_model_files_with(
            &requests[0],
            &requests[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| {
                generation.store(2, Ordering::Release);
                proposals(b)
            },
            || false,
        );
        assert!(matches!(e, Err(ModelComparisonError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    let provider_phase = Cell::new(false);
    let e = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, b, cancel| {
            provider_phase.set(true);
            assert!(cancel());
            provider_phase.set(false);
            proposals(b)
        },
        || provider_phase.get(),
    );
    assert!(matches!(e, Err(ModelComparisonError::Cancelled)));
    assert_eq!(budget.used(), 0);
}
#[test]
fn policy_preflight_and_proposal_count_refuse_without_partial_evidence() {
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let missing = DecodeRequest::new("/nonexistent/rrrah-model-compare.bmp");
    let mut p = policy();
    p.sampling.seed = 0;
    let e = compare_model_files_with(
        &missing,
        &missing,
        Path::new("/nonexistent/model"),
        p,
        &budget,
        |_, _, _, _, _| panic!("provider before policy preflight"),
        || false,
    );
    assert!(matches!(e, Err(ModelComparisonError::Policy)));
    assert_eq!(budget.peak(), 0);
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let e = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, b, _| {
            Ok(b.try_buffer(
                17,
                ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 1.,
                },
            )?
            .freeze())
        },
        || false,
    );
    assert!(matches!(
        e,
        Err(ModelComparisonError::Geometry(
            rrrah_dedup::geometry::GeometryError::Budget
        ))
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn model_mutation_during_fresh_pixel_confirmation_discards_supported_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let provider_done = Cell::new(false);
    let changed = Cell::new(false);
    let result = compare_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, b, _| {
            let p = proposals(b)?;
            provider_done.set(true);
            Ok(p)
        },
        || {
            // Retained input/proposal payload stays below 32 KiB. Crossing it after
            // provider completion occurs when fresh original-pixel decode allocates.
            if provider_done.get() && budget.used() > 32768 && !changed.replace(true) {
                let mut bytes = std::fs::read(&w).unwrap();
                bytes.push(0);
                std::fs::write(&w, bytes).unwrap();
            }
            false
        },
    );
    assert!(changed.get());
    assert!(
        matches!(
            result,
            Err(ModelComparisonError::ModelSource(
                rrrah_dedup::exact::SnapshotError::Changed
            ))
        ),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn global_and_both_request_tokens_cancel_mid_and_final_publication() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    assert!(
        compare_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            }
        )
        .unwrap()
        .supported()
    );
    for at in [calls.get() / 2, calls.get()] {
        let count = Cell::new(0);
        assert!(matches!(
            compare_model_files_with(
                &r[0],
                &r[1],
                &w,
                policy(),
                &budget,
                |_, _, _, b, _| proposals(b),
                || {
                    count.set(count.get() + 1);
                    count.get() == at
                }
            ),
            Err(ModelComparisonError::Cancelled)
        ));
        assert_eq!(count.get(), at);
        assert_eq!(budget.used(), 0);
        for owner in 0..2 {
            let generation = Arc::new(AtomicU64::new(1));
            let mut requests = r.clone();
            requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
            let count = Cell::new(0);
            assert!(matches!(
                compare_model_files_with(
                    &requests[0],
                    &requests[1],
                    &w,
                    policy(),
                    &budget,
                    |_, _, _, b, _| proposals(b),
                    || {
                        count.set(count.get() + 1);
                        if count.get() == at {
                            generation.store(2, Ordering::Release);
                        }
                        false
                    }
                ),
                Err(ModelComparisonError::Cancelled)
            ));
            assert_eq!(count.get(), at);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn provider_cancellation_error_is_typed_cancelled_for_global_and_both_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let (requests, weights) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for mode in 0..3 {
        let generation = Arc::new(AtomicU64::new(1));
        let mut requests = requests.clone();
        if mode != 0 {
            requests[mode - 1].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        }
        let once = Cell::new(false);
        let result = compare_model_files_with(
            &requests[0],
            &requests[1],
            &weights,
            policy(),
            &budget,
            |_, _, _, _, cancel| {
                if mode == 0 {
                    once.set(true);
                } else {
                    generation.store(2, Ordering::Release);
                }
                assert!(cancel());
                // A cooperating provider returns its own interruption error.
                // The library must preserve the caller's cancellation outcome.
                Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "provider cancelled").into())
            },
            || once.replace(false),
        );
        assert!(
            matches!(result, Err(ModelComparisonError::Cancelled)),
            "mode {mode}: {result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}

#[test]
fn joint_model_projection_admission_precedes_io_and_cancellation_has_priority() {
    use rrrah_dedup::model_compare::{JointModelComparisonPolicy, compare_model_joint_files_with};
    let request = DecodeRequest::new("missing-input");
    let budget = MemoryBudget::new(0);
    let mut p = policy();
    for max_points in [16, usize::MAX] {
        p.geometry.max_points = max_points;
        for cancelled in [false, true] {
            let result = compare_model_joint_files_with(&request, &request, Path::new("missing-model"),
                JointModelComparisonPolicy { comparison:p, maximum_projection_checks:0 }, &budget,
                |_,_,_,_,_| panic!("provider must not run"), || cancelled);
            if cancelled { assert!(matches!(result,Err(ModelComparisonError::Cancelled))); }
            else { assert!(matches!(result,Err(ModelComparisonError::Geometry(rrrah_dedup::geometry::GeometryError::Budget)))); }
            assert_eq!(budget.used(),0);
        }
    }
}

#[test]
fn joint_model_keeps_original_pixel_confirmation_and_releases_on_cancelled_retry() {
    use rrrah_dedup::model_compare::{JointModelComparisonPolicy, compare_model_joint_files_with};
    let dir = tempfile::tempdir().unwrap(); let (r,w)=fixture(dir.path());
    let budget=MemoryBudget::new(8*1024*1024);
    let p=JointModelComparisonPolicy {comparison:policy(),maximum_projection_checks:2*16*65};
    for cancel_provider in [false,true,false] {
        let cancelled=Cell::new(false);
        let result=compare_model_joint_files_with(&r[0],&r[1],&w,p,&budget,
            |_,_,_,b,_| {let points=proposals(b)?;cancelled.set(cancel_provider);Ok(points)},||cancelled.get());
        if cancel_provider {assert!(matches!(result,Err(ModelComparisonError::Cancelled)));}
        else {let evidence=result.unwrap();assert!(evidence.supported());assert_eq!(evidence.geometry_inliers,10);}
        assert_eq!(budget.used(),0);
    }
}

#[test]
fn joint_shifted_crop_domains_confirm_the_same_original_pixels_without_a_translation_error() {
    use rrrah_dedup::model_compare::compare_model_joint_regions_files_with;
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let e = compare_model_joint_regions_files_with(
        &r[0],
        &r[1],
        &w,
        [16, 8, 32, 32],
        [8, 16, 32, 32],
        rrrah_dedup::model_compare::JointModelComparisonPolicy { comparison: policy(), maximum_projection_checks: 2*16*65 },
        &budget,
        |_, a, b, budget, _| {
            assert_eq!(a.dimensions, [32, 32]);
            assert_eq!(a.original, [32, 32]);
            assert_eq!(b.dimensions, [32, 32]);
            region_proposals(budget)
        },
        || false,
    )
    .unwrap();
    assert!(e.supported(), "{e:?}");
    assert_eq!(e.geometry_inliers, 10);
    let h = e.transform.unwrap().matrix;
    for (y, row) in h.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            assert!((value - if x == y { 1. } else { 0. }).abs() < 1e-8);
        }
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn joint_region_lifecycle_refuses_whole_source_mutation_and_both_request_cancellations() {
    use rrrah_dedup::model_compare::compare_model_joint_regions_files_with;
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let result = compare_model_joint_regions_files_with(
            &r[0],
            &r[1],
            &w,
            [16, 8, 32, 32],
            [8, 16, 32, 32],
            rrrah_dedup::model_compare::JointModelComparisonPolicy { comparison: policy(), maximum_projection_checks: 2*16*65 },
            &budget,
            |_, _, _, b, _| {
                let mut changed = original.clone();
                changed.push(0);
                std::fs::write(path, changed)?;
                region_proposals(b)
            },
            || false,
        );
        assert!(
            matches!(
                result,
                Err(ModelComparisonError::Left(_)
                    | ModelComparisonError::Right(_)
                    | ModelComparisonError::ModelSource(_))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
    for owner in 0..2 {
        let mut requests = r.clone();
        let generation = Arc::new(AtomicU64::new(1));
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let result = compare_model_joint_regions_files_with(
            &requests[0],
            &requests[1],
            &w,
            [16, 8, 32, 32],
            [8, 16, 32, 32],
            rrrah_dedup::model_compare::JointModelComparisonPolicy { comparison: policy(), maximum_projection_checks: 2*16*65 },
            &budget,
            |_, _, _, b, cancel| {
                generation.store(2, Ordering::Release);
                assert!(cancel());
                generation.store(1, Ordering::Release);
                region_proposals(b)
            },
            || false,
        );
        assert!(matches!(result, Err(ModelComparisonError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn joint_final_caller_checkpoint_cannot_publish_a_model_result_after_source_or_weight_mutation() {
    use rrrah_dedup::model_compare::compare_model_joint_files_with;
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    drop(
        compare_model_joint_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy { comparison: policy(), maximum_projection_checks: 2*16*65 },
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let last = calls.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let current = Cell::new(0);
        let e = compare_model_joint_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy { comparison: policy(), maximum_projection_checks: 2*16*65 },
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                current.set(current.get() + 1);
                if current.get() == last {
                    let mut changed = original.clone();
                    changed.push(0);
                    std::fs::write(path, changed).unwrap();
                }
                false
            },
        );
        assert!(
            matches!(
                e,
                Err(ModelComparisonError::Left(_)
                    | ModelComparisonError::Right(_)
                    | ModelComparisonError::ModelSource(_))
            ),
            "{e:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
}

#[test]
fn extracted_crop_bank_retains_mapped_points_and_last_clone_credit_without_geometry() {
 use rrrah_dedup::model_compare::extract_model_region_points_files_with as extract;
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);
 let bank=extract(&r[0],&r[1],&w,[0,0,32,32],[16,8,32,32],policy(),&b,|_,_,_,b,_|proposals(b),||false).unwrap();
 assert_eq!(bank.original_dimensions,[[64,64],[64,64]]);assert_eq!(bank.raw_proposals,10);assert_eq!(bank.points.len(),6);
 for p in bank.points.iter(){assert_eq!(p.target[0]-p.source[0],16.);assert_eq!(p.target[1]-p.source[1],8.);}
 let held=bank.clone();drop(bank);assert!(b.used()>0);drop(held);assert_eq!(b.used(),0);
}
#[test]
fn extracted_bank_refuses_changed_source_or_weights_after_provider() {
 use rrrah_dedup::model_compare::extract_model_region_points_files_with as extract;
 for which in 0..3 {let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);let changed=if which==2 {w.clone()} else {r[which].path.clone()};
 let result=extract(&r[0],&r[1],&w,[0,0,64,64],[0,0,64,64],policy(),&b,|_,_,_,b,_|{let p=proposals(b)?;std::fs::write(&changed,b"changed")?;Ok(p)},||false);assert!(result.is_err());assert_eq!(b.used(),0);}
}
#[test]
fn extracted_bank_empty_is_explicit_and_every_cancel_checkpoint_releases_credit() {
 use rrrah_dedup::model_compare::extract_model_region_points_files_with as extract;
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);
 let empty=extract(&r[0],&r[1],&w,[0,0,64,64],[0,0,64,64],policy(),&b,|_,_,_,b,_|Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze()),||false).unwrap();assert!(empty.points.is_empty());drop(empty);assert_eq!(b.used(),0);
 let calls=Cell::new(0);let bank=extract(&r[0],&r[1],&w,[0,0,64,64],[0,0,64,64],policy(),&b,|_,_,_,b,_|proposals(b),||{calls.set(calls.get()+1);false}).unwrap();drop(bank);
 for at in 1..=calls.get(){let n=Cell::new(0);let result=extract(&r[0],&r[1],&w,[0,0,64,64],[0,0,64,64],policy(),&b,|_,_,_,b,_|proposals(b),||{n.set(n.get()+1);n.get()>=at});assert!(matches!(result,Err(ModelComparisonError::Cancelled)));assert_eq!(b.used(),0);}
}

fn file_bank_policy()->rrrah_dedup::model_banks::ModelBankSearchPolicy {
 use rrrah_dedup::{geometry::JointProjectivePolicy,joint_witnesses::{JointBankPolicy,JointWitnessPolicy},model_tiles::ModelTileWorkLimits};
 let mut p=policy();p.anchors.local.maximum_points=400;p.anchors.local.maximum_point_checks=100000;p.anchors.maximum_selection_checks=250000;let n=25*p.proposals.max_proposals as u64;let fit=2*p.geometry.max_points as u64*(p.sampling.trials+1);let sep=n*(n-1)/2;let mut hypothesis=p.geometry;hypothesis.min_inliers=4;
 let b=JointBankPolicy{hypothesis:JointProjectivePolicy{target:hypothesis,source_tolerance:p.anchors.local.source_tolerance,maximum_projection_checks:fit},sampling:p.sampling,witnesses:JointWitnessPolicy{source_tolerance:p.anchors.local.source_tolerance,target_tolerance:p.geometry.tolerance,minimum_separation:2.,maximum_points:n as usize,maximum_projection_checks:2*n,maximum_separation_checks:sep},minimum_witnesses:10,maximum_banks:25,maximum_total_projection_checks:25*(fit+2*n),maximum_total_separation_checks:25*sep};let mut work=ModelTileWorkLimits::for_model(p).unwrap();work.model_input_reads=p.input.max_pixel_reads*52;
 rrrah_dedup::model_banks::ModelBankSearchPolicy{comparison:p,banks:b,work}
}
#[test]
fn file_bank_search_extracts_all_windows_and_confirms_original_pixels() {
 use rrrah_dedup::model_banks::search_model_banks_files_with as search;
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);let calls=Cell::new(0);
 let result=search(&r[0],&r[1],&w,file_bank_policy(),&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();assert_eq!(calls.get(),25);assert_eq!(result.attempts,25);assert!(result.supported());assert!(result.comparison.geometry_inliers>=10);assert_eq!(b.used(),0);
}
#[test]
fn file_bank_work_refuses_before_io_and_late_source_change_discards_banks() {
 use rrrah_dedup::{model_banks::search_model_banks_files_with as search,model_tiles::ModelTileError};
 let r=DecodeRequest::new("missing");let b=MemoryBudget::new(1_000_000);let mut p=file_bank_policy();p.banks.maximum_total_projection_checks-=1;let result=search(&r,&r,Path::new("missing"),p,&b,&mut |_,_,_,_,_|panic!("preflight"),||false);assert!(matches!(result,Err(ModelTileError::Work)));assert_eq!(b.used(),0);
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let calls=Cell::new(0);let result=search(&r[0],&r[1],&w,file_bank_policy(),&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);let v=proposals(b)?;if calls.get()==25{std::fs::write(&w,b"mutated weights")?;}Ok(v)},||false);assert!(result.is_err());assert_eq!(calls.get(),25);assert_eq!(b.used(),0);
}
#[test]
fn file_bank_empty_is_inconclusive_and_cancelled_provider_discards_retained_prefix() {
 use rrrah_dedup::{model_banks::search_model_banks_files_with as search,model_tiles::ModelTileError};
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);
 let empty=search(&r[0],&r[1],&w,file_bank_policy(),&b,&mut |_,_,_,b,_|Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze()),||false).unwrap();assert_eq!(empty.diagnostics,Default::default());assert_eq!(empty.pixel_attempts,0);assert_eq!(empty.attempts,25);assert!(!empty.supported());assert!(empty.comparison.transform.is_none());assert_eq!(b.used(),0);
 let calls=Cell::new(0);let stop=Cell::new(false);let result=search(&r[0],&r[1],&w,file_bank_policy(),&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);let p=proposals(b)?;if calls.get()==5 {stop.set(true);}Ok(p)},||stop.get());assert!(matches!(result,Err(ModelTileError::Cancelled)));assert_eq!(b.used(),0);
}

#[test]
fn file_bank_pixels_refuse_flat_or_inverted_even_with_consistent_provider_geometry() {
 use rrrah_dedup::model_banks::search_model_banks_files_with as search;
 for invert in [false,true] {
  let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());
  for (side,request) in r.iter().enumerate() {
   let mut bytes=std::fs::read(&request.path).unwrap();
   for y in 0..64 {for x in 0..64 {let value=if invert {40+((x*13+y*7)%160) as u8}else{100};let value=if invert && side==1 {255-value}else{value};let at=54+(y*64+x)*3;bytes[at..at+3].fill(value);}}
   std::fs::write(&request.path,bytes).unwrap();
  }
  let b=MemoryBudget::new(1_000_000);let calls=Cell::new(0);
  let result=search(&r[0],&r[1],&w,file_bank_policy(),&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();
  assert_eq!(calls.get(),25);assert!(!result.supported(),"invert={invert}: {result:?}");assert!(result.comparison.transform.is_none());assert_eq!(b.used(),0);
 }
}

#[test]
fn file_bank_coverage_recovers_after_insufficient_windows_without_repeating_inference() {
 use rrrah_dedup::model_banks::{search_model_banks_files_with as single,search_model_banks_coverage_files_with as search};
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);let p=file_bank_policy();
 let old=single(&r[0],&r[1],&w,p,&b,&mut |_,_,_,b,_|proposals(b),||false).unwrap();let same=search(&r[0],&r[1],&w,p,&[1],&b,&mut |_,_,_,b,_|proposals(b),||false).unwrap();assert_eq!(format!("{old:?}"),format!("{same:?}"));
 let mut p=p;p.work.point_checks*=2;p.work.selection_checks*=2;p.work.rank_sites*=2;p.work.pixel_reads*=2;let calls=Cell::new(0);
 let result=search(&r[0],&r[1],&w,p,&[12,1],&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();
 assert!(result.supported());assert_eq!(result.pixel_window_radius,Some(1));assert!(result.pixel_attempts>=2 && result.pixel_attempts<=50);assert_eq!(calls.get(),25);assert_eq!(b.used(),0);
}
#[test]
fn file_bank_coverage_admits_all_radii_before_io_and_cancel_has_priority() {
 use rrrah_dedup::{model_banks::search_model_banks_coverage_files_with as search,model_tiles::ModelTileError};
 let r=DecodeRequest::new("missing");let b=MemoryBudget::new(1_000_000);let p=file_bank_policy();
 for radii in [vec![],vec![1;9],vec![u32::MAX],vec![1,2]] {
  let result=search(&r,&r,Path::new("missing"),p,&radii,&b,&mut |_,_,_,_,_|panic!("preflight"),||false);assert!(matches!(result,Err(ModelTileError::Work)|Err(ModelTileError::Comparison(ModelComparisonError::Policy))));assert_eq!(b.used(),0);
 }
 let result=search(&r,&r,Path::new("missing"),p,&[],&b,&mut |_,_,_,_,_|panic!("cancel"),||true);assert!(matches!(result,Err(ModelTileError::Cancelled)));assert_eq!(b.used(),0);
}

#[test]
fn file_bank_coverage_keeps_flat_and_inverted_pixels_inconclusive() {
 use rrrah_dedup::model_banks::search_model_banks_coverage_files_with as search;
 for invert in [false,true] {
  let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());
  for (side,request) in r.iter().enumerate() {
   let mut bytes=std::fs::read(&request.path).unwrap();
   for y in 0..64 {for x in 0..64 {let value=if invert {40+((x*13+y*7)%160) as u8}else{100};let value=if invert && side==1 {255-value}else{value};let at=54+(y*64+x)*3;bytes[at..at+3].fill(value);}}
   std::fs::write(&request.path,bytes).unwrap();
  }
  let mut p=file_bank_policy();p.work.point_checks*=2;p.work.selection_checks*=2;p.work.rank_sites*=2;p.work.pixel_reads*=2;
  let b=MemoryBudget::new(1_000_000);let calls=Cell::new(0);let result=search(&r[0],&r[1],&w,p,&[2,6],&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();
  assert_eq!(calls.get(),25);assert!(!result.supported(),"invert={invert}: {result:?}");assert!(result.pixel_window_radius.is_none());assert!(result.pixel_attempts>=2 && result.pixel_attempts<=50);assert!(result.diagnostics.geometry_candidates>0);assert_eq!(result.pixel_attempts,result.diagnostics.pixel_geometry_refusals+result.diagnostics.pixel_rejections);if invert {assert!(result.diagnostics.pixel_rejections>0);}else{assert!(result.diagnostics.pixel_geometry_refusals>0);}assert_eq!(b.used(),0);
 }
}

#[test]
fn file_bank_pixel_contexts_share_extraction_and_report_selected_context() {
 use rrrah_dedup::model_banks::{ModelBankPixelRecipe as Recipe,search_model_banks_pixel_coverage_files_with as search,search_model_banks_coverage_files_with as legacy};
 let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let b=MemoryBudget::new(1_000_000);let mut p=file_bank_policy();
 let old=legacy(&r[0],&r[1],&w,p,&[1],&b,&mut |_,_,_,b,_|proposals(b),||false).unwrap();
 let same=search(&r[0],&r[1],&w,p,&[Recipe{window_radius:1,rank_radius:p.comparison.anchors.local.rank.radius}],&b,&mut |_,_,_,b,_|proposals(b),||false).unwrap();assert_eq!(format!("{old:?}"),format!("{same:?}"));
 p.work.point_checks*=2;p.work.selection_checks*=2;p.work.rank_sites*=2;p.work.pixel_reads*=2;let calls=Cell::new(0);
 let result=search(&r[0],&r[1],&w,p,&[Recipe{window_radius:12,rank_radius:8},Recipe{window_radius:1,rank_radius:2}],&b,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();
 assert!(result.supported());assert_eq!(result.pixel_window_radius,Some(1));assert_eq!(result.pixel_rank_radius,Some(2));assert!(result.pixel_attempts>=2);assert_eq!(calls.get(),25);assert_eq!(b.used(),0);
}

#[test]
fn file_bank_pixel_context_preflight_rejects_overflow_work_and_cancel_before_io() {
 use rrrah_dedup::{model_banks::{ModelBankPixelRecipe as Recipe,search_model_banks_pixel_coverage_files_with as search},model_tiles::ModelTileError};
 let r=DecodeRequest::new("missing");let b=MemoryBudget::new(1_000_000);let p=file_bank_policy();
 for recipes in [vec![],vec![Recipe{window_radius:1,rank_radius:2};9],vec![Recipe{window_radius:1,rank_radius:0}],vec![Recipe{window_radius:1,rank_radius:u32::MAX}],vec![Recipe{window_radius:1,rank_radius:2};2]] {
  let result=search(&r,&r,Path::new("missing"),p,&recipes,&b,&mut |_,_,_,_,_|panic!("preflight"),||false);assert!(matches!(result,Err(ModelTileError::Work)|Err(ModelTileError::Comparison(ModelComparisonError::Policy))));assert_eq!(b.used(),0);
 }
 let result=search(&r,&r,Path::new("missing"),p,&[],&b,&mut |_,_,_,_,_|panic!("cancel"),||true);assert!(matches!(result,Err(ModelTileError::Cancelled)));assert_eq!(b.used(),0);
}

#[test]
fn file_bank_multiple_rank_contexts_do_not_confirm_flat_or_inverted_pixels() {
 use rrrah_dedup::model_banks::{ModelBankPixelRecipe as Recipe,search_model_banks_pixel_coverage_files_with as search};
 for invert in [false,true] {
  let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());
  for (side,request) in r.iter().enumerate() {
   let mut bytes=std::fs::read(&request.path).unwrap();
   for y in 0..64 {for x in 0..64 {let value=if invert {40+((x*13+y*7)%160) as u8}else{100};let value=if invert && side==1 {255-value}else{value};let at=54+(y*64+x)*3;bytes[at..at+3].fill(value);}}
   std::fs::write(&request.path,bytes).unwrap();
  }
  let mut p=file_bank_policy();p.work.point_checks*=4;p.work.selection_checks*=4;p.work.rank_sites*=4;p.work.pixel_reads*=4;
  let recipes=[Recipe{window_radius:2,rank_radius:8},Recipe{window_radius:6,rank_radius:8},Recipe{window_radius:2,rank_radius:2},Recipe{window_radius:6,rank_radius:2}];
  let budget=MemoryBudget::new(1_000_000);let calls=Cell::new(0);let result=search(&r[0],&r[1],&w,p,&recipes,&budget,&mut |_,_,_,b,_|{calls.set(calls.get()+1);proposals(b)},||false).unwrap();
  assert_eq!(calls.get(),25);assert!(!result.supported(),"invert={invert}: {result:?}");assert!(result.pixel_window_radius.is_none() && result.pixel_rank_radius.is_none());assert!(result.pixel_attempts>=4);assert_eq!(budget.used(),0);
 }
}

#[test]
fn file_bank_multiple_contexts_discard_last_provider_source_change_and_cancel() {
 use rrrah_dedup::{model_banks::{ModelBankPixelRecipe as Recipe,search_model_banks_pixel_coverage_files_with as search},model_tiles::ModelTileError};
 for cancelled in [false,true] {
  let dir=tempfile::tempdir().unwrap();let (r,w)=fixture(dir.path());let mut p=file_bank_policy();p.work.point_checks*=2;p.work.selection_checks*=2;p.work.rank_sites*=2;p.work.pixel_reads*=2;let calls=Cell::new(0);let stop=Cell::new(false);let budget=MemoryBudget::new(1_000_000);
  let result=search(&r[0],&r[1],&w,p,&[Recipe{window_radius:1,rank_radius:8},Recipe{window_radius:1,rank_radius:2}],&budget,&mut |_,_,_,b,_|{calls.set(calls.get()+1);let points=proposals(b);if calls.get()==25 {std::fs::write(&w,b"changed weights").unwrap();stop.set(cancelled);}points},||stop.get());
  assert!(result.is_err());if cancelled {assert!(matches!(result,Err(ModelTileError::Cancelled)));}assert_eq!(calls.get(),25);assert_eq!(budget.used(),0);
 }
}

fn overlap_proposals(index:usize,budget:&MemoryBudget)->Result<SharedBuffer<ScoredProposal>,ProposalProviderError>{
 let mut p=budget.try_buffer(10,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?;
 for(i,point)in p.iter_mut().enumerate(){let xy=[4.+(i%5)as f64*5.,8.+(i/5)as f64*8.];point.source=xy;point.target=[xy[0]+(index%3)as f64*16.,xy[1]+(index/3)as f64*16.];}Ok(p.freeze())
}
#[test]
fn overlap_exposure_search_shares_nine_banks_and_retains_selected_metadata(){
 use rrrah_dedup::model_banks::{search_model_banks_overlap_exposure_files_with as search,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let b=MemoryBudget::new(2_000_000);let calls=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 let result=search(&r[0],&r[1],&w,file_bank_policy(),&recipes,6*4096*25,&b,&mut|_,left,right,b,_|{assert_eq!(left.original,[32,32]);assert_eq!(right.original,[64,64]);let i=calls.get();calls.set(i+1);overlap_proposals(i,b)},||false).unwrap();
 assert_eq!(calls.get(),9);assert_eq!(result.search.attempts,9);assert!(result.search.supported());assert_eq!(result.search.pixel_window_radius,Some(1));assert_eq!(result.search.pixel_rank_radius,Some(2));let e=result.exposures.unwrap();assert_eq!(e[0],e[1]);assert_eq!(e[0].pixel_reads,12288);assert_eq!(b.used(),0);
}
#[test]
fn overlap_exposure_admits_cumulative_calibration_before_io_and_cancel_wins(){
 use rrrah_dedup::{model_banks::{search_model_banks_overlap_exposure_files_with as search,ModelBankPixelRecipe},model_tiles::ModelTileError};
 let r=DecodeRequest::new("missing-overlap-exposure-source");let b=MemoryBudget::new(1000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 assert!(matches!(search(&r,&r,Path::new("missing"),file_bank_policy(),&recipes,6*4096*25-1,&b,&mut|_,_,_,_,_|panic!("preflight"),||false),Err(ModelTileError::Work)));
 assert!(matches!(search(&r,&r,Path::new("missing"),file_bank_policy(),&recipes,0,&b,&mut|_,_,_,_,_|panic!("cancelled"),||true),Err(ModelTileError::Cancelled)));assert_eq!(b.used(),0);
}
#[test]
fn overlap_exposure_last_provider_mutation_and_owner_cancel_discard_all_banks(){
 use rrrah_dedup::{model_banks::{search_model_banks_overlap_exposure_files_with as search,ModelBankPixelRecipe},model_tiles::ModelTileError};
 for owner in [None,Some(0),Some(1)]{let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let generation=Arc::new(AtomicU64::new(1));if let Some(i)=owner{r[i].cancellation=Some(GenerationToken::new(generation.clone(),1));}let b=MemoryBudget::new(2_000_000);let n=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 let result=search(&r[0],&r[1],&w,file_bank_policy(),&recipes,6*4096*25,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);let p=overlap_proposals(i,b)?;if i==8{if owner.is_some(){generation.store(2,Ordering::Relaxed);}else{std::fs::write(&w,b"changed after last overlap provider")?;}}Ok(p)},||false);
 assert!(result.is_err());if owner.is_some(){assert!(matches!(result,Err(ModelTileError::Cancelled)));}assert_eq!(n.get(),9);assert_eq!(b.used(),0);}
}
#[test]
fn overlap_exposure_flat_and_inverted_pixels_keep_compact_miss_unconfirmed(){
 use rrrah_dedup::model_banks::{search_model_banks_overlap_exposure_files_with as search,ModelBankPixelRecipe};
 for invert in [false,true]{let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());for(side,request)in r.iter().enumerate(){let mut bytes=std::fs::read(&request.path).unwrap();for y in 0..64{for x in 0..64{let v=if invert{40+((x*13+y*7)%160)as u8}else{100};let v=if invert&&side==1{255-v}else{v};let at=54+(y*64+x)*3;bytes[at..at+3].fill(v);}}std::fs::write(&request.path,bytes).unwrap();}
 let b=MemoryBudget::new(2_000_000);let n=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let e=search(&r[0],&r[1],&w,file_bank_policy(),&recipes,6*4096*25,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);overlap_proposals(i,b)},||false).unwrap();assert_eq!(n.get(),9);assert!(!e.search.supported());assert!(e.search.comparison.transform.is_none());assert!(e.exposures.is_none());assert_eq!(b.used(),0);}
}

fn exposure_recovery_policy()->rrrah_dedup::model_banks::ModelBankExposureRecoveryPolicy{
 rrrah_dedup::model_banks::ModelBankExposureRecoveryPolicy::for_stages(file_bank_policy(),file_bank_policy(),6*4096*25).unwrap()
}
#[test]
fn exposure_recovery_preserves_primary_support_and_skips_secondary_provider(){
 use rrrah_dedup::model_banks::{search_model_banks_exposure_recovery_files_with as search,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let b=MemoryBudget::new(2_000_000);let n=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 let e=search(&r[0],&r[1],&w,exposure_recovery_policy(),&recipes,&recipes,&b,&mut|_,_,_,b,_|{n.set(n.get()+1);assert!(n.get()<=25);proposals(b)},||false).unwrap();assert!(e.supported());assert!(e.primary.supported());assert!(e.recovery.is_none());assert_eq!(n.get(),25);assert_eq!(b.used(),0);
}
#[test]
fn exposure_recovery_advances_only_after_primary_miss_with_one_shared_provider(){
 use rrrah_dedup::model_banks::{search_model_banks_exposure_recovery_files_with as search,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let b=MemoryBudget::new(2_000_000);let n=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 let e=search(&r[0],&r[1],&w,exposure_recovery_policy(),&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if i<25{Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze())}else{overlap_proposals(i-25,b)}},||false).unwrap();assert!(!e.primary.supported());let r=e.recovery.unwrap();assert!(r.search.supported());assert!(r.exposures.is_some());assert_eq!(n.get(),34);assert_eq!(b.used(),0);
}
#[test]
fn exposure_recovery_validates_suffix_cumulative_caps_and_overflow_before_io(){
 use rrrah_dedup::{model_banks::{search_model_banks_exposure_recovery_files_with as search,ModelBankPixelRecipe,ModelBankExposureRecoveryPolicy},model_tiles::ModelTileError};
 let r=DecodeRequest::new("missing-exposure-recovery");let b=MemoryBudget::new(1000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 for calibration in [false,true]{let mut p=exposure_recovery_policy();if calibration{p.maximum_calibration_pixel_reads-=1;}else{p.work.model_input_reads-=1;}
 assert!(matches!(search(&r,&r,Path::new("missing"),p,&recipes,&recipes,&b,&mut|_,_,_,_,_|panic!("preflight"),||false),Err(ModelTileError::Work)));}
 let bad=[ModelBankPixelRecipe{window_radius:1,rank_radius:0}];assert!(search(&r,&r,Path::new("missing"),exposure_recovery_policy(),&recipes,&bad,&b,&mut|_,_,_,_,_|panic!("suffix"),||false).is_err());
 let mut p=file_bank_policy();p.work.pixel_reads=u64::MAX;assert!(matches!(ModelBankExposureRecoveryPolicy::for_stages(p,p,6*4096*25),Err(ModelTileError::Work)));assert_eq!(b.used(),0);
}
#[test]
fn exposure_recovery_last_secondary_mutation_and_both_owners_are_atomic(){
 use rrrah_dedup::{model_banks::{search_model_banks_exposure_recovery_files_with as search,ModelBankPixelRecipe},model_tiles::ModelTileError};
 for owner in [None,Some(0),Some(1)]{let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let generation=Arc::new(AtomicU64::new(1));if let Some(i)=owner{r[i].cancellation=Some(GenerationToken::new(generation.clone(),1));}let b=MemoryBudget::new(2_000_000);let n=Cell::new(0);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 let result=search(&r[0],&r[1],&w,exposure_recovery_policy(),&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if i<25{return Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze());}let p=overlap_proposals(i-25,b)?;if i==33{if owner.is_some(){generation.store(2,Ordering::Relaxed);}else{std::fs::write(&r[0].path,b"source replaced after last secondary provider")?;}}Ok(p)},||false);
 assert!(result.is_err());if owner.is_some(){assert!(matches!(result,Err(ModelTileError::Cancelled)));}assert_eq!(n.get(),34);assert_eq!(b.used(),0);}
}
#[test]
fn exposure_recovery_primary_success_still_checks_final_outer_weights_and_owners(){
 use rrrah_dedup::{model_banks::{search_model_banks_exposure_recovery_files_with as search,ModelBankPixelRecipe},model_tiles::ModelTileError};
 let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];
 for owner in [None,Some(0),Some(1)]{let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let b=MemoryBudget::new(2_000_000);let calls=Cell::new(0);search(&r[0],&r[1],&w,exposure_recovery_policy(),&recipes,&recipes,&b,&mut|_,_,_,b,_|proposals(b),||{calls.set(calls.get()+1);false}).unwrap();let stop=calls.get();let generation=Arc::new(AtomicU64::new(1));if let Some(i)=owner{r[i].cancellation=Some(GenerationToken::new(generation.clone(),1));}let n=Cell::new(0);
 let result=search(&r[0],&r[1],&w,exposure_recovery_policy(),&recipes,&recipes,&b,&mut|_,_,_,b,_|proposals(b),||{n.set(n.get()+1);if n.get()==stop{if owner.is_some(){generation.store(2,Ordering::Relaxed);}else{std::fs::write(&w,b"changed on final outer caller checkpoint").unwrap();}}false});assert!(result.is_err());if owner.is_some(){assert!(matches!(result,Err(ModelTileError::Cancelled)));}assert_eq!(b.used(),0);}
}

#[test]
fn exposure_collection_preserves_primary_support_scheduling_and_owned_report_credit(){
 use rrrah_dedup::model_banks::{scan_model_banks_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let third=dir.path().join("third.bmp");std::fs::copy(&r[0].path,&third).unwrap();let files=[(30,DecodeRequest::new(third)),(10,r[0].clone()),(20,r[1].clone())];let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_pairs(exposure_recovery_policy(),3,3).unwrap();
 let schedule=rrrah_dedup::pair_schedule::CompletePairSchedule::new(&[30,10,20],&[(20,30)],3,3,1,&b,||false).unwrap();
 let n=Cell::new(0);let e=scan(files,Some(&schedule),&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{n.set(n.get()+1);proposals(b)},||false).unwrap();
 assert_eq!(e.file_ids(),[10,20,30]);assert_eq!(e.pairs().iter().map(|p|(p.left,p.right)).collect::<Vec<_>>(),[(10,20),(10,30),(20,30)]);assert!(e.pairs().iter().all(|p|p.evidence.primary.supported()&&p.evidence.recovery.is_none()));assert_eq!(n.get(),75);drop(schedule);assert!(b.used()>0);drop(e);assert_eq!(b.used(),0);
}
#[test]
fn exposure_collection_keeps_unknown_pairs_and_recovers_with_shared_provider(){
 use rrrah_dedup::model_banks::{scan_model_banks_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_pairs(exposure_recovery_policy(),2,1).unwrap();
 for recover in [false,true]{let n=Cell::new(0);let e=scan([(1,r[0].clone()),(2,r[1].clone())],None,&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if recover&&i>=25{overlap_proposals(i-25,b)}else{Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze())}},||false).unwrap();assert_eq!(n.get(),34);let pair=&e.pairs()[0].evidence;assert!(!pair.primary.supported());assert_eq!(pair.supported(),recover);assert_eq!(pair.recovery.unwrap().search.attempts,9);assert_eq!(pair.recovery.unwrap().exposures.is_some(),recover);drop(e);assert_eq!(b.used(),0);}
}
#[test]
fn exposure_collection_admits_all_totals_suffix_and_storage_before_iteration(){
 use rrrah_dedup::{model_banks::{scan_model_banks_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe},collection::CollectionError as E};
 let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let base=ModelBankExposureCollectionPolicy::for_pairs(exposure_recovery_policy(),3,3).unwrap();
 for field in 0..9{let mut p=base;match field{0=>p.work.model_input_reads=0,1=>p.work.proposal_ranking_checks=0,2=>p.work.proposal_separation_checks=0,3=>p.work.geometry_hypotheses=0,4=>p.work.point_checks=0,5=>p.work.selection_checks=0,6=>p.work.rank_sites=0,7=>p.work.pixel_reads=0,_=>p.maximum_calibration_pixel_reads=0};assert!(matches!(scan(std::iter::from_fn(||panic!("iterator consumed")),None,Path::new("missing"),p,&recipes,&recipes,&b,&mut|_,_,_,_,_|panic!("provider"),||false),Err(E::Work)));assert_eq!(b.used(),0);}
 let bad=[ModelBankPixelRecipe{window_radius:1,rank_radius:0}];assert!(matches!(scan(std::iter::from_fn(||panic!("iterator")),None,Path::new("missing"),base,&recipes,&bad,&b,&mut|_,_,_,_,_|panic!("provider"),||false),Err(E::Policy(_))));
 let tiny=MemoryBudget::new(1);assert!(matches!(scan(std::iter::from_fn(||panic!("iterator")),None,Path::new("missing"),base,&recipes,&recipes,&tiny,&mut|_,_,_,_,_|panic!("provider"),||false),Err(E::Memory(_))));assert_eq!(tiny.used(),0);
 assert!(matches!(scan(std::iter::from_fn(||panic!("iterator")),None,Path::new("missing"),base,&bad,&bad,&b,&mut|_,_,_,_,_|panic!("provider"),||true),Err(E::Cancelled)));
 let mut overflowing=exposure_recovery_policy();overflowing.work.model_input_reads=u64::MAX;assert!(ModelBankExposureCollectionPolicy::for_pairs(overflowing,2,2).is_err());
}
#[test]
fn exposure_collection_final_pair_mutation_of_prior_source_and_owner_discards_batch(){
 use rrrah_dedup::{model_banks::{scan_model_banks_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe},collection::CollectionError as E};
 for owner in [false,true]{let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let prior=r[0].path.clone();let third=dir.path().join("third.bmp");std::fs::copy(&prior,&third).unwrap();let generation=Arc::new(AtomicU64::new(1));r[0].cancellation=Some(GenerationToken::new(generation.clone(),1));let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_pairs(exposure_recovery_policy(),3,3).unwrap();let n=Cell::new(0);
 let e=scan([(1,r[0].clone()),(2,r[1].clone()),(3,DecodeRequest::new(third))],None,&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{n.set(n.get()+1);let result=proposals(b)?;if n.get()==75{if owner{generation.store(2,Ordering::Release);}else{std::fs::write(&prior,b"changed prior source")?;}}Ok(result)},||false);
 assert_eq!(n.get(),75);if owner{assert!(matches!(e,Err(E::Cancelled)));}else{assert!(matches!(e,Err(E::Source{id:1,..})));}assert_eq!(b.used(),0);}
}

#[test]
fn bidirectional_exposure_collection_preserves_direction_and_skips_supported_suffix(){
 use rrrah_dedup::model_banks::{scan_model_banks_bidirectional_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe};
 let dir=tempfile::tempdir().unwrap();let(r,w)=fixture(dir.path());let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_bidirectional_pairs(exposure_recovery_policy(),2,1).unwrap();
 for mode in 0..4 {let n=Cell::new(0);let e=scan([(2,r[1].clone()),(1,r[0].clone())],None,&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if mode==0 || (mode==1&&i>=34&&i<59){proposals(b)}else if mode==2&&i>=59{overlap_proposals(i-59,b)}else{Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze())}},||false).unwrap();
 let pair=&e.pairs()[0];assert_eq!((pair.left,pair.right),(1,2));let v=pair.evidence;assert_eq!(v.supported(),mode!=3);assert_eq!(v.forward.supported(),mode==0);assert_eq!(v.reverse.is_none(),mode==0);assert_eq!(n.get(),[25,59,68,68][mode]);if mode==2{assert!(v.reverse.unwrap().recovery.unwrap().search.supported());}assert!(b.used()>0);drop(e);assert_eq!(b.used(),0);}
}
#[test]
fn bidirectional_exposure_collection_prepays_reverse_before_input(){
 use rrrah_dedup::{model_banks::{scan_model_banks_bidirectional_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe},collection::CollectionError as E};
 let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let full=ModelBankExposureCollectionPolicy::for_bidirectional_pairs(exposure_recovery_policy(),2,1).unwrap();let half=ModelBankExposureCollectionPolicy::for_pairs(exposure_recovery_policy(),2,1).unwrap();
 for field in 0..9{let mut p=full;match field{0=>p.work.model_input_reads=half.work.model_input_reads,1=>p.work.proposal_ranking_checks=half.work.proposal_ranking_checks,2=>p.work.proposal_separation_checks=half.work.proposal_separation_checks,3=>p.work.geometry_hypotheses=half.work.geometry_hypotheses,4=>p.work.point_checks=half.work.point_checks,5=>p.work.selection_checks=half.work.selection_checks,6=>p.work.rank_sites=half.work.rank_sites,7=>p.work.pixel_reads=half.work.pixel_reads,_=>p.maximum_calibration_pixel_reads=half.maximum_calibration_pixel_reads};assert!(matches!(scan(std::iter::from_fn(||panic!("input consumed")),None,Path::new("missing"),p,&recipes,&recipes,&b,&mut|_,_,_,_,_|panic!("provider"),||false),Err(E::Work)));assert_eq!(b.used(),0);}
 assert!(ModelBankExposureCollectionPolicy::for_bidirectional_pairs(exposure_recovery_policy(),2,usize::MAX).is_err());
}
#[test]
fn bidirectional_exposure_collection_reverse_final_mutation_and_cancel_discard_batch(){
 use rrrah_dedup::{model_banks::{scan_model_banks_bidirectional_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe},collection::CollectionError as E};
 for owner in [false,true]{let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let generation=Arc::new(AtomicU64::new(1));r[0].cancellation=Some(GenerationToken::new(generation.clone(),1));let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_bidirectional_pairs(exposure_recovery_policy(),2,1).unwrap();let n=Cell::new(0);
 let e=scan([(1,r[0].clone()),(2,r[1].clone())],None,&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if i==67{if owner{generation.store(2,Ordering::Release);}else{std::fs::write(&w,b"changed during final reverse attempt")?;}}Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze())},||false);
 assert_eq!(n.get(),68);assert!(e.is_err());if owner{assert!(matches!(e,Err(E::Cancelled)));}assert_eq!(b.used(),0);}
}

#[test]
fn bidirectional_exposure_final_reverse_invalidates_previously_supported_pairs(){
 use rrrah_dedup::{model_banks::{scan_model_banks_bidirectional_exposure_recovery_collection_with as scan,ModelBankExposureCollectionPolicy,ModelBankPixelRecipe},collection::CollectionError as E};
 for mode in 0..4 {let dir=tempfile::tempdir().unwrap();let(mut r,w)=fixture(dir.path());let prior=r[0].path.clone();let third=dir.path().join("third.bmp");std::fs::copy(&prior,&third).unwrap();let generation=Arc::new(AtomicU64::new(1));r[0].cancellation=Some(GenerationToken::new(generation.clone(),1));let b=MemoryBudget::new(4_000_000);let recipes=[ModelBankPixelRecipe{window_radius:1,rank_radius:2}];let p=ModelBankExposureCollectionPolicy::for_bidirectional_pairs(exposure_recovery_policy(),3,3).unwrap();let n=Cell::new(0);let stop=Cell::new(false);
 let result=scan([(3,DecodeRequest::new(third)),(1,r[0].clone()),(2,r[1].clone())],None,&w,p,&recipes,&recipes,&b,&mut|_,_,_,b,_|{let i=n.get();n.set(i+1);if i<50{return proposals(b);}if i==117 {match mode{0=>std::fs::write(&prior,b"earlier supported source replaced")?,1=>generation.store(2,Ordering::Release),2=>stop.set(true),_=>return Err(std::io::Error::other("reverse provider failed after two supported pairs").into())}}Ok(b.try_buffer(0,ScoredProposal{source:[0.;2],target:[0.;2],confidence:1.})?.freeze())},||stop.get());
 assert_eq!(n.get(),118);match mode{0=>assert!(matches!(result,Err(E::Source{id:1,..}))),1|2=>assert!(matches!(result,Err(E::Cancelled))),_=>assert!(matches!(result,Err(E::Pair{left:2,right:3,..})))}assert_eq!(b.used(),0);
 }
}
