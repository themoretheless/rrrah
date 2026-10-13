#![cfg(feature = "decode")]
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::{ModelComparisonPolicy, ProposalProviderError},
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
        p.target = [p.source[0] + 32., p.source[1]];
    }
    Ok(points.freeze())
}

use rrrah_dedup::model_tiles::{ModelTileError, ModelTileWorkLimits, search_model_tiles_files_with};
fn empty(budget: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
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
}

#[test]
fn geometry_without_original_pixel_support_does_not_stop_the_tile_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut inverted = std::fs::read(&r[1].path).unwrap();
    for v in &mut inverted[54..] {
        *v = 255 - *v;
    }
    std::fs::write(&r[1].path, inverted).unwrap();
    let p = policy();
    let calls = Cell::new(0);
    let evidence = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 { proposals(b) } else { empty(b) }
        },
        || false,
    )
    .unwrap();
    assert_eq!(calls.get(), 25);
    assert!(!evidence.supported());
    assert_eq!(
        evidence.attempts()[0]
            .evidence
            .comparison()
            .unwrap()
            .geometry_inliers,
        10
    );
    assert!(
        evidence.attempts()[0]
            .evidence
            .comparison()
            .unwrap()
            .pixels
            .is_some_and(|p| !p.supported)
    );
    drop(evidence);
    assert_eq!(budget.used(), 0);
}

#[test]
fn tiny_decoded_domains_omit_empty_quadrants_and_duplicate_full_regions() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let original = std::fs::read(&r[0].path).unwrap();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    for (height, attempts) in [(1u32, 1usize), (7, 9)] {
        let mut bmp = original[..54].to_vec();
        bmp[2..6].copy_from_slice(&(54 + height * 4).to_le_bytes());
        bmp[18..22].copy_from_slice(&1u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&height.to_le_bytes());
        bmp.extend(std::iter::repeat_n(0u8, height as usize * 4));
        for request in &r {
            std::fs::write(&request.path, &bmp).unwrap();
        }
        let calls = Cell::new(0);
        let evidence = search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                empty(b)
            },
            || false,
        )
        .unwrap();
        assert_eq!(calls.get(), attempts);
        assert_eq!(evidence.attempts().len(), attempts);
        assert_eq!(
            evidence.attempts()[0]
                .evidence
                .comparison()
                .unwrap()
                .original_dimensions,
            [[1, height]; 2]
        );
        drop(evidence);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn complete_fixed_order_visits_all_twenty_five_regions_and_shared_credit_follows_last_owner() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let evidence = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            empty(b)
        },
        || false,
    )
    .unwrap();
    assert_eq!(calls.get(), 25);
    assert_eq!(evidence.attempts().len(), 25);
    assert!(!evidence.supported());
    let rectangles = [
        [0, 0, 64, 64],
        [0, 0, 32, 32],
        [32, 0, 32, 32],
        [0, 32, 32, 32],
        [32, 32, 32, 32],
    ];
    for (i, a) in evidence.attempts().iter().enumerate() {
        assert_eq!(a.left_rectangle, rectangles[i / 5]);
        assert_eq!(a.right_rectangle, rectangles[i % 5]);
        assert_eq!(
            a.evidence.comparison().unwrap().original_dimensions,
            [[64, 64]; 2]
        );
        assert!(!a.evidence.supported());
    }
    let retained = budget.used();
    assert!(retained > 0);
    let clone = evidence.clone();
    drop(evidence);
    assert_eq!(budget.used(), retained);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn whole_support_stops_at_one_and_late_crop_support_stops_only_after_original_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    for stop in [1, 11] {
        let calls = Cell::new(0);
        let evidence = search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, a, b, budget, _| {
                calls.set(calls.get() + 1);
                assert!(calls.get() <= stop);
                if calls.get() == stop {
                    if stop == 1 {
                        proposals(budget)
                    } else {
                        assert_eq!(a.original, [32, 32]);
                        assert_eq!(b.original, [64, 64]);
                        region_proposals(budget)
                    }
                } else {
                    empty(budget)
                }
            },
            || false,
        )
        .unwrap();
        assert_eq!(calls.get(), stop);
        assert_eq!(evidence.attempts().len(), stop);
        assert!(evidence.supported(), "{evidence:?}");
        assert_eq!(
            evidence
                .terminal()
                .evidence
                .comparison()
                .unwrap()
                .geometry_inliers,
            10
        );
        if stop == 11 {
            assert_eq!(evidence.terminal().left_rectangle, [32, 0, 32, 32]);
            assert_eq!(evidence.terminal().right_rectangle, [0, 0, 64, 64]);
        }
        drop(evidence);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn all_eight_work_caps_overflow_invalid_policy_and_storage_refuse_before_io() {
    let p = policy();
    let work = ModelTileWorkLimits::for_model(p).unwrap();
    let r = DecodeRequest::new("missing");
    for field in 0..8 {
        let mut limits = work;
        *match field {
            0 => &mut limits.model_input_reads,
            1 => &mut limits.proposal_ranking_checks,
            2 => &mut limits.proposal_separation_checks,
            3 => &mut limits.geometry_hypotheses,
            4 => &mut limits.point_checks,
            5 => &mut limits.selection_checks,
            6 => &mut limits.rank_sites,
            _ => &mut limits.pixel_reads,
        } -= 1;
        let budget = MemoryBudget::new(0);
        assert!(matches!(
            search_model_tiles_files_with(
                &r,
                &r,
                Path::new("missing"),
                p,
                limits,
                &budget,
                |_, _, _, _, _| panic!("provider must not run"),
                || false
            ),
            Err(ModelTileError::Work)
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }
    let mut overflow = p;
    overflow.input.max_pixel_reads = u64::MAX;
    assert!(matches!(
        ModelTileWorkLimits::for_model(overflow),
        Err(ModelTileError::Work)
    ));
    let budget = MemoryBudget::new(0);
    assert!(matches!(
        search_model_tiles_files_with(
            &r,
            &r,
            Path::new("missing"),
            p,
            work,
            &budget,
            |_, _, _, _, _| panic!("provider must not run"),
            || false
        ),
        Err(ModelTileError::Memory(_))
    ));
    let mut invalid = p;
    invalid.sampling.seed = 0;
    assert!(matches!(
        search_model_tiles_files_with(
            &r,
            &r,
            Path::new("missing"),
            invalid,
            work,
            &budget,
            |_, _, _, _, _| panic!("provider must not run"),
            || false
        ),
        Err(ModelTileError::Comparison(_))
    ));
    assert!(matches!(
        search_model_tiles_files_with(
            &r,
            &r,
            Path::new("missing"),
            invalid,
            work,
            &budget,
            |_, _, _, _, _| panic!("provider must not run"),
            || true
        ),
        Err(ModelTileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}
#[test]
fn late_provider_failures_source_mutations_and_both_generations_discard_all_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    let work = ModelTileWorkLimits::for_model(p).unwrap();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let calls = Cell::new(0);
        let e = search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 7 {
                    let mut changed = original.clone();
                    changed.push(0);
                    std::fs::write(path, changed)?;
                }
                empty(b)
            },
            || false,
        );
        assert!(matches!(
            e,
            Err(ModelTileError::Source(_) | ModelTileError::Comparison(_))
        ));
        assert_eq!(calls.get(), 7);
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
    for owner in 0..2 {
        let mut requests = r.clone();
        let generation = Arc::new(AtomicU64::new(1));
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let calls = Cell::new(0);
        let e = search_model_tiles_files_with(
            &requests[0],
            &requests[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, cancel| {
                calls.set(calls.get() + 1);
                if calls.get() == 7 {
                    generation.store(2, Ordering::Release);
                    assert!(cancel());
                    generation.store(1, Ordering::Release);
                }
                empty(b)
            },
            || false,
        );
        assert!(matches!(e, Err(ModelTileError::Cancelled)));
        assert_eq!(calls.get(), 7);
        assert_eq!(budget.used(), 0);
    }
    let calls = Cell::new(0);
    let e = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        work,
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 7 {
                Err(std::io::Error::other("late provider failure").into())
            } else {
                empty(b)
            }
        },
        || false,
    );
    assert!(matches!(e, Err(ModelTileError::Comparison(_))));
    assert_eq!(budget.used(), 0);
}
#[test]
fn final_caller_checkpoint_mutation_cancellation_and_request_change_never_publish() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    let work = ModelTileWorkLimits::for_model(p).unwrap();
    let calls = Cell::new(0);
    drop(
        search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
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
        let e = search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
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
        assert!(matches!(e, Err(ModelTileError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
    let current = Cell::new(0);
    assert!(matches!(
        search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ModelTileError::Cancelled)
    ));
    for owner in 0..2 {
        let mut requests = r.clone();
        let generation = Arc::new(AtomicU64::new(1));
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let current = Cell::new(0);
        assert!(matches!(
            search_model_tiles_files_with(
                &requests[0],
                &requests[1],
                &w,
                p,
                work,
                &budget,
                |_, _, _, b, _| empty(b),
                || {
                    current.set(current.get() + 1);
                    if current.get() == last {
                        generation.store(2, Ordering::Release);
                    }
                    false
                }
            ),
            Err(ModelTileError::Cancelled)
        ));
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn repeated_regions_reuse_identical_managed_inputs_and_retained_owners_keep_credit() {
    use rrrah_dedup::{
        model_input_file::prepare_gray_model_region_file,
        model_tiles::{ModelTileWorkLimits, search_model_tiles_files_with},
    };
    let dir = tempfile::tempdir().unwrap();
    let (requests, weights) = fixture(dir.path());
    let p = policy();
    let rectangles = [
        [0, 0, 64, 64],
        [0, 0, 32, 32],
        [32, 0, 32, 32],
        [0, 32, 32, 32],
        [32, 32, 32, 32],
    ];
    let expected_budget = MemoryBudget::new(1_000_000);
    let expected: [[rrrah_dedup::model_input::GrayModelInput; 5]; 2] = std::array::from_fn(|side| {
        std::array::from_fn(|index| {
            prepare_gray_model_region_file(
                &requests[side],
                rectangles[index],
                p.decode,
                p.input,
                &expected_budget,
                || false,
            )
            .unwrap()
            .input()
            .clone()
        })
    });
    let budget = MemoryBudget::new(1_000_000);
    let mut left_owners: [Option<SharedBuffer<u8>>; 5] = std::array::from_fn(|_| None);
    let mut right_owners: [Option<SharedBuffer<u8>>; 5] = std::array::from_fn(|_| None);
    let mut calls = 0;
    let report = search_model_tiles_files_with(
        &requests[0],
        &requests[1],
        &weights,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, left, right, budget, _| {
            assert!(calls < 25);
            let (i, j) = (calls / 5, calls % 5);
            calls += 1;
            for (input, reference) in [(left, &expected[0][i]), (right, &expected[1][j])] {
                assert_eq!(input.dimensions, reference.dimensions);
                assert_eq!(input.original, reference.original);
                assert_eq!(&input.pixels[..], &reference.pixels[..]);
            }
            for (owner, input) in [(&mut left_owners[i], left), (&mut right_owners[j], right)] {
                if let Some(held) = owner {
                    assert_eq!(
                        held.as_ptr(),
                        input.pixels.as_ptr(),
                        "an unchanged region was prepared again"
                    );
                } else {
                    *owner = Some(input.pixels.clone());
                }
            }
            empty(budget)
        },
        || false,
    )
    .unwrap();
    assert_eq!(calls, 25);
    assert_eq!(report.attempts().len(), 25);
    drop(report);
    let retained_bytes: usize = left_owners
        .iter()
        .chain(&right_owners)
        .map(|owner| owner.as_ref().unwrap().len())
        .sum();
    assert_eq!(budget.used(), retained_bytes as u64);
    drop(left_owners);
    drop(right_owners);
    assert_eq!(budget.used(), 0);
    drop(expected);
    assert_eq!(expected_budget.used(), 0);
}

#[test]
fn late_preparation_memory_refusal_discards_cached_inputs_and_all_attempts() {
    use rrrah_dedup::model_tiles::{ModelTileError, ModelTileWorkLimits, search_model_tiles_files_with};
    let dir = tempfile::tempdir().unwrap();
    let (requests, weights) = fixture(dir.path());
    let p = policy();
    let budget = MemoryBudget::new(1_000_000);
    let mut held = None;
    let mut calls = 0;
    let error = search_model_tiles_files_with(
        &requests[0],
        &requests[1],
        &weights,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, budget, _| {
            calls += 1;
            if calls == 5 {
                held = Some(budget.try_reserve(budget.available_bytes())?);
            }
            empty(budget)
        },
        || false,
    )
    .unwrap_err();
    assert!(!matches!(error, ModelTileError::Cancelled));
    assert_eq!(calls, 5, "the next new region must refuse before provider work");
    assert_eq!(budget.peak(), budget.limit());
    assert_eq!(budget.used(), held.as_ref().unwrap().bytes());
    drop(held);
    assert_eq!(budget.used(), 0);
}

fn insufficient_proposals(
    budget: &MemoryBudget,
) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut points = budget.try_buffer(
        10,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in points.iter_mut().enumerate() {
        p.source = [1. + 2.5 * (i % 5) as f64, 1. + 2.5 * (i / 5) as f64];
        p.target = p.source;
    }
    Ok(points.freeze())
}
fn small_region_proposals(
    budget: &MemoryBudget,
) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut points = budget.try_buffer(
        10,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in points.iter_mut().enumerate() {
        p.source = [4. + 5. * (i % 5) as f64, 4. + 12. * (i / 5) as f64];
        p.target = p.source;
    }
    Ok(points.freeze())
}
#[test]
fn insufficient_anchor_geometry_keeps_the_attempt_and_recovers_on_a_later_rectangle() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let e = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                insufficient_proposals(b)
            } else {
                small_region_proposals(b)
            }
        },
        || false,
    )
    .unwrap();
    assert!(e.supported());
    assert_eq!(calls.get(), 2);
    assert_eq!(e.attempts().len(), 2);
    assert_eq!(e.insufficient_geometry_attempts(), 1);
    assert!(matches!(
        e.attempts()[0].evidence,
        rrrah_dedup::model_tiles::ModelTileOutcome::InsufficientAnchorGeometry
    ));
    assert_eq!(e.terminal().left_rectangle, [0, 0, 64, 64]);
    assert_eq!(e.terminal().right_rectangle, [0, 0, 32, 32]);
    assert!(
        e.terminal()
            .evidence
            .comparison()
            .unwrap()
            .pixels
            .unwrap()
            .supported
    );
    drop(e);
    assert_eq!(budget.used(), 0);
}
#[test]
fn exhausted_insufficient_geometry_is_typed_and_retains_the_complete_managed_roster() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let result = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                insufficient_proposals(b)
            } else {
                empty(b)
            }
        },
        || false,
    );
    let Err(ModelTileError::InsufficientAnchorGeometry(e)) = result else {
        panic!("expected typed exhaustion: {result:?}")
    };
    assert_eq!(calls.get(), 25);
    assert_eq!(e.attempts().len(), 25);
    assert_eq!(e.insufficient_geometry_attempts(), 1);
    assert!(!e.supported());
    assert_ne!(e.model_digest(), [0; 32]);
    assert!(
        e.attempts()[1..]
            .iter()
            .all(|a| a.evidence.comparison().is_some_and(|e| !e.supported()))
    );
    let clone = e.clone();
    drop(e);
    assert!(budget.used() > 0);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn insufficient_geometry_does_not_swallow_a_later_proposal_work_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let result = search_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                insufficient_proposals(b)
            } else {
                Ok(b.try_buffer(
                    17,
                    ScoredProposal {
                        source: [4.; 2],
                        target: [4.; 2],
                        confidence: 1.,
                    },
                )?
                .freeze())
            }
        },
        || false,
    );
    assert!(matches!(
        result,
        Err(ModelTileError::Comparison(
            rrrah_dedup::model_compare::ModelComparisonError::Geometry(
                rrrah_dedup::geometry::GeometryError::Budget
            )
        ))
    ));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.used(), 0);
}
#[test]
fn final_source_mutation_takes_precedence_over_exhausted_insufficient_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    let run = |checkpoint: &dyn Fn() -> bool| {
        let calls = Cell::new(0);
        search_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    insufficient_proposals(b)
                } else {
                    empty(b)
                }
            },
            checkpoint,
        )
    };
    let count = Cell::new(0);
    let baseline = run(&|| {
        count.set(count.get() + 1);
        false
    });
    assert!(matches!(
        baseline,
        Err(ModelTileError::InsufficientAnchorGeometry(_))
    ));
    drop(baseline);
    assert_eq!(budget.used(), 0);
    let last = count.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let n = Cell::new(0);
        let result = run(&|| {
            n.set(n.get() + 1);
            if n.get() == last {
                let mut bytes = original.clone();
                bytes.push(0);
                std::fs::write(path, bytes).unwrap();
            }
            false
        });
        assert!(matches!(result, Err(ModelTileError::Source(_))), "{result:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
}

#[test]
fn joint_complete_fixed_order_visits_all_twenty_five_regions_and_shared_credit_follows_last_owner() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let evidence = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        rrrah_dedup::model_compare::JointModelComparisonPolicy {
            comparison: p,
            maximum_projection_checks: 2 * 16 * 65,
        },
        2 * 16 * 65 * 25,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            empty(b)
        },
        || false,
    )
    .unwrap();
    assert_eq!(calls.get(), 25);
    assert_eq!(evidence.attempts().len(), 25);
    assert!(!evidence.supported());
    let rectangles = [
        [0, 0, 64, 64],
        [0, 0, 32, 32],
        [32, 0, 32, 32],
        [0, 32, 32, 32],
        [32, 32, 32, 32],
    ];
    for (i, a) in evidence.attempts().iter().enumerate() {
        assert_eq!(a.left_rectangle, rectangles[i / 5]);
        assert_eq!(a.right_rectangle, rectangles[i % 5]);
        assert_eq!(
            a.evidence.comparison().unwrap().original_dimensions,
            [[64, 64]; 2]
        );
        assert!(!a.evidence.supported());
    }
    let retained = budget.used();
    assert!(retained > 0);
    let clone = evidence.clone();
    drop(evidence);
    assert_eq!(budget.used(), retained);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn joint_whole_support_stops_at_one_and_late_crop_support_stops_only_after_original_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    for stop in [1, 11] {
        let calls = Cell::new(0);
        let evidence = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy {
                comparison: p,
                maximum_projection_checks: 2 * 16 * 65,
            },
            2 * 16 * 65 * 25,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, a, b, budget, _| {
                calls.set(calls.get() + 1);
                assert!(calls.get() <= stop);
                if calls.get() == stop {
                    if stop == 1 {
                        proposals(budget)
                    } else {
                        assert_eq!(a.original, [32, 32]);
                        assert_eq!(b.original, [64, 64]);
                        region_proposals(budget)
                    }
                } else {
                    empty(budget)
                }
            },
            || false,
        )
        .unwrap();
        assert_eq!(calls.get(), stop);
        assert_eq!(evidence.attempts().len(), stop);
        assert!(evidence.supported(), "{evidence:?}");
        assert_eq!(
            evidence
                .terminal()
                .evidence
                .comparison()
                .unwrap()
                .geometry_inliers,
            10
        );
        if stop == 11 {
            assert_eq!(evidence.terminal().left_rectangle, [32, 0, 32, 32]);
            assert_eq!(evidence.terminal().right_rectangle, [0, 0, 64, 64]);
        }
        drop(evidence);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn joint_final_caller_checkpoint_mutation_cancellation_and_request_change_never_publish() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    let work = ModelTileWorkLimits::for_model(p).unwrap();
    let calls = Cell::new(0);
    drop(
        rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy {
                comparison: p,
                maximum_projection_checks: 2 * 16 * 65,
            },
            2 * 16 * 65 * 25,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
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
        let e = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy {
                comparison: p,
                maximum_projection_checks: 2 * 16 * 65,
            },
            2 * 16 * 65 * 25,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
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
        assert!(matches!(e, Err(ModelTileError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
    let current = Cell::new(0);
    assert!(matches!(
        rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy {
                comparison: p,
                maximum_projection_checks: 2 * 16 * 65
            },
            2 * 16 * 65 * 25,
            work,
            &budget,
            |_, _, _, b, _| empty(b),
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ModelTileError::Cancelled)
    ));
    for owner in 0..2 {
        let mut requests = r.clone();
        let generation = Arc::new(AtomicU64::new(1));
        requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let current = Cell::new(0);
        assert!(matches!(
            rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
                &requests[0],
                &requests[1],
                &w,
                rrrah_dedup::model_compare::JointModelComparisonPolicy {
                    comparison: p,
                    maximum_projection_checks: 2 * 16 * 65
                },
                2 * 16 * 65 * 25,
                work,
                &budget,
                |_, _, _, b, _| empty(b),
                || {
                    current.set(current.get() + 1);
                    if current.get() == last {
                        generation.store(2, Ordering::Release);
                    }
                    false
                }
            ),
            Err(ModelTileError::Cancelled)
        ));
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn joint_projection_total_is_admitted_before_io_even_if_first_tile_could_support() {
    let r = DecodeRequest::new("missing");
    let budget = MemoryBudget::new(0);
    let p = policy();
    for (per, total) in [
        (2 * 16 * 65 - 1, u64::MAX),
        (2 * 16 * 65, 2 * 16 * 65 * 25 - 1),
        (u64::MAX, u64::MAX),
    ] {
        for stop in [false, true] {
            let result = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
                &r,
                &r,
                Path::new("missing-weights"),
                rrrah_dedup::model_compare::JointModelComparisonPolicy {
                    comparison: p,
                    maximum_projection_checks: per,
                },
                total,
                ModelTileWorkLimits::for_model(p).unwrap(),
                &budget,
                |_, _, _, _, _| panic!("no provider before work admission"),
                || stop,
            );
            if stop {
                assert!(matches!(result, Err(ModelTileError::Cancelled)));
            } else {
                assert!(matches!(result, Err(ModelTileError::Work)));
            }
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn joint_exhausted_insufficient_geometry_is_typed_and_retains_the_complete_managed_roster() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let result = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        rrrah_dedup::model_compare::JointModelComparisonPolicy {
            comparison: p,
            maximum_projection_checks: 2 * 16 * 65,
        },
        2 * 16 * 65 * 25,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                insufficient_proposals(b)
            } else {
                empty(b)
            }
        },
        || false,
    );
    let Err(ModelTileError::InsufficientAnchorGeometry(e)) = result else {
        panic!("expected typed exhaustion: {result:?}")
    };
    assert_eq!(calls.get(), 25);
    assert_eq!(e.attempts().len(), 25);
    assert_eq!(e.insufficient_geometry_attempts(), 1);
    assert!(!e.supported());
    assert_ne!(e.model_digest(), [0; 32]);
    assert!(
        e.attempts()[1..]
            .iter()
            .all(|a| a.evidence.comparison().is_some_and(|e| !e.supported()))
    );
    let clone = e.clone();
    drop(e);
    assert!(budget.used() > 0);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn joint_insufficient_geometry_does_not_swallow_a_later_proposal_work_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let p = policy();
    let result = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
        &r[0],
        &r[1],
        &w,
        rrrah_dedup::model_compare::JointModelComparisonPolicy {
            comparison: p,
            maximum_projection_checks: 2 * 16 * 65,
        },
        2 * 16 * 65 * 25,
        ModelTileWorkLimits::for_model(p).unwrap(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                insufficient_proposals(b)
            } else {
                Ok(b.try_buffer(
                    17,
                    ScoredProposal {
                        source: [4.; 2],
                        target: [4.; 2],
                        confidence: 1.,
                    },
                )?
                .freeze())
            }
        },
        || false,
    );
    assert!(matches!(
        result,
        Err(ModelTileError::Comparison(
            rrrah_dedup::model_compare::ModelComparisonError::Geometry(
                rrrah_dedup::geometry::GeometryError::Budget
            )
        ))
    ));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.used(), 0);
}
#[test]
fn joint_final_source_mutation_takes_precedence_over_exhausted_insufficient_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let p = policy();
    let run = |checkpoint: &dyn Fn() -> bool| {
        let calls = Cell::new(0);
        rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r[0],
            &r[1],
            &w,
            rrrah_dedup::model_compare::JointModelComparisonPolicy {
                comparison: p,
                maximum_projection_checks: 2 * 16 * 65,
            },
            2 * 16 * 65 * 25,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    insufficient_proposals(b)
                } else {
                    empty(b)
                }
            },
            checkpoint,
        )
    };
    let count = Cell::new(0);
    let baseline = run(&|| {
        count.set(count.get() + 1);
        false
    });
    assert!(matches!(
        baseline,
        Err(ModelTileError::InsufficientAnchorGeometry(_))
    ));
    drop(baseline);
    assert_eq!(budget.used(), 0);
    let last = count.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path).unwrap();
        let n = Cell::new(0);
        let result = run(&|| {
            n.set(n.get() + 1);
            if n.get() == last {
                let mut bytes = original.clone();
                bytes.push(0);
                std::fs::write(path, bytes).unwrap();
            }
            false
        });
        assert!(matches!(result, Err(ModelTileError::Source(_))), "{result:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original).unwrap();
    }
}

#[test]
fn joint_source_tolerance_square_overflow_and_underflow_refuse_before_io() {
    use rrrah_dedup::model_compare::{
        JointModelComparisonPolicy, ModelComparisonError, compare_model_joint_files_with,
    };
    let r = DecodeRequest::new("missing");
    let budget = MemoryBudget::new(0);
    for tolerance in [1e300, 1e-300] {
        let mut p = policy();
        p.anchors.local.source_tolerance = tolerance;
        let joint = JointModelComparisonPolicy {
            comparison: p,
            maximum_projection_checks: 2 * 16 * 65,
        };
        let single = compare_model_joint_files_with(
            &r,
            &r,
            Path::new("missing"),
            joint,
            &budget,
            |_, _, _, _, _| panic!("preflight"),
            || false,
        );
        assert!(matches!(single, Err(ModelComparisonError::Policy)));
        let tiles = rrrah_dedup::model_tiles::search_joint_model_tiles_files_with(
            &r,
            &r,
            Path::new("missing"),
            joint,
            2 * 16 * 65 * 25,
            ModelTileWorkLimits::for_model(p).unwrap(),
            &budget,
            |_, _, _, _, _| panic!("preflight"),
            || false,
        );
        assert!(matches!(
            tiles,
            Err(ModelTileError::Comparison(ModelComparisonError::Policy))
        ));
        assert_eq!(budget.used(), 0);
    }
}
