#![cfg(feature = "decode")]
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    model_compare::ProposalProviderError,
    model_input::ModelInputPolicy,
    model_projected_compare::{
        ProjectedModelError, ProjectedModelPolicy, compare_projected_model_files_with,
    },
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
#[test]
fn hdr_original_proof_accepts_copy_and_rejects_equal_projection_collision() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    let run = || {
        compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, a, b, budget, _| {
                assert_eq!(&*a.pixels, &*b.pixels);
                proposals(budget)
            },
            || false,
        )
        .unwrap()
    };
    let e = run();
    assert!(e.supported);
    assert_eq!(e.geometry_inliers, 6);
    assert_eq!(e.pixels.unwrap().forward.matched_pixels, 64);
    assert_eq!(budget.used(), 0);
    std::fs::write(&r[1].path, pfm(4.01)).unwrap();
    let e = run();
    assert!(!e.supported);
    let p = e.pixels.unwrap();
    assert_eq!((p.forward.matched_pixels, p.reverse.matched_pixels), (0, 0));
    assert_eq!(budget.used(), 0);
}
#[test]
fn work_and_invalid_thresholds_refuse_before_provider() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    let called = Cell::new(false);
    let mut p = policy();
    p.pixels.max_source_pixels = 127;
    let e = compare_projected_model_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        &budget,
        |_, _, _, b, _| {
            called.set(true);
            proposals(b)
        },
        || false,
    );
    assert!(matches!(
        e,
        Err(ProjectedModelError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert!(!called.get());
    assert_eq!(budget.used(), 0);
    p = policy();
    p.minimum_coverage = 0.;
    let missing = DecodeRequest::new(dir.path().join("missing"));
    assert!(matches!(
        compare_projected_model_files_with(
            &missing,
            &missing,
            &w,
            p,
            &budget,
            |_, _, _, _, _| panic!(),
            || false
        ),
        Err(ProjectedModelError::Policy)
    ));
}
#[test]
fn source_and_weights_mutation_in_provider_never_publish_evidence() {
    for mutate_weights in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (r, w) = fixture(dir.path());
        let budget = MemoryBudget::new(65536);
        let e = compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| {
                if mutate_weights {
                    std::fs::write(&w, b"changed weights").unwrap();
                } else {
                    std::fs::write(&r[0].path, pfm(4.01)).unwrap();
                }
                proposals(b)
            },
            || false,
        );
        assert!(matches!(
            e,
            Err(ProjectedModelError::Left(_)) | Err(ProjectedModelError::ModelSource(_))
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn cancellation_and_final_checkpoint_mutations_discard_result() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    let count = Cell::new(0);
    let e = compare_projected_model_files_with(
        &r[0],
        &r[1],
        &w,
        policy(),
        &budget,
        |_, _, _, b, _| proposals(b),
        || {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(e.supported);
    let total = count.get();
    assert!(total > 100);
    for stop in [1, total / 2, total] {
        let n = Cell::new(0);
        let e = compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                n.set(n.get() + 1);
                n.get() == stop
            },
        );
        assert!(
            matches!(e, Err(ProjectedModelError::Cancelled)),
            "checkpoint {stop}: {e:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    for weights in [false, true] {
        std::fs::write(&r[0].path, pfm(4.)).unwrap();
        std::fs::write(&w, b"fixture weights").unwrap();
        let n = Cell::new(0);
        let e = compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| proposals(b),
            || {
                n.set(n.get() + 1);
                if n.get() == total {
                    if weights {
                        std::fs::write(&w, b"changed weights").unwrap();
                    } else {
                        std::fs::write(&r[0].path, pfm(4.01)).unwrap();
                    }
                }
                false
            },
        );
        assert!(
            matches!(
                e,
                Err(ProjectedModelError::Left(_)) | Err(ProjectedModelError::ModelSource(_))
            ),
            "{e:?}"
        );
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn transparent_black_collision_is_rejected_by_original_alpha() {
    let dir = tempfile::tempdir().unwrap();
    let (_, w) = fixture(dir.path());
    let requests: [DecodeRequest; 2] = std::array::from_fn(|i| {
        let path = dir.path().join(format!("alpha-{i}.qoi"));
        let mut b = b"qoif".to_vec();
        b.extend(8u32.to_be_bytes());
        b.extend(8u32.to_be_bytes());
        b.extend([4, 1]);
        for _ in 0..64 {
            b.extend([0xff, 0, 0, 0, if i == 0 { 64 } else { 255 }]);
        }
        b.extend([0, 0, 0, 0, 0, 0, 0, 1]);
        std::fs::write(&path, b).unwrap();
        DecodeRequest::new(path)
    });
    let budget = MemoryBudget::new(65536);
    let e = compare_projected_model_files_with(
        &requests[0],
        &requests[1],
        &w,
        policy(),
        &budget,
        |_, a, b, budget, _| {
            assert_eq!(&*a.pixels, &*b.pixels);
            proposals(budget)
        },
        || false,
    )
    .unwrap();
    assert!(e.transform.is_some());
    assert!(!e.supported);
    let pixels = e.pixels.unwrap();
    assert_eq!(
        (pixels.forward.matched_pixels, pixels.reverse.matched_pixels),
        (0, 0)
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn each_request_generation_and_tiny_memory_refusal_release_all_owners() {
    use rrrah_decode::GenerationToken;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..2 {
        let dir = tempfile::tempdir().unwrap();
        let (mut r, w) = fixture(dir.path());
        let generation = Arc::new(AtomicU64::new(1));
        r[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
        let budget = MemoryBudget::new(65536);
        let e = compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, b, _| {
                generation.store(2, Ordering::Release);
                proposals(b)
            },
            || false,
        );
        assert!(matches!(e, Err(ProjectedModelError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(1);
    assert!(
        compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            policy(),
            &budget,
            |_, _, _, _, _| panic!("provider called despite refusal"),
            || false
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn identity_candidate_recovers_noisy_self_geometry_without_loosening_pixel_policy() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    let mut p = policy();
    p.geometry.tolerance = 0.05;
    let e = compare_projected_model_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        &budget,
        |_, _, _, b, _| {
            let mut points = b.try_buffer(
                6,
                ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 1.,
                },
            )?;
            for (i, q) in points.iter_mut().enumerate() {
                q.source = [1. + 2. * (i % 3) as f64, 1. + 4. * (i / 3) as f64];
                q.target = [q.source[0] + 0.02, q.source[1] + 0.01];
            }
            Ok(points.freeze())
        },
        || false,
    )
    .unwrap();
    assert!(e.supported);
    assert_eq!(e.pixel_attempts, 1);
    assert_eq!(e.pixel_work, 128);
    assert_eq!(
        e.transform.unwrap().matrix,
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
    );
    assert_eq!(e.pixels.unwrap().forward.matched_pixels, 64);
    assert_eq!(budget.used(), 0);
}

#[test]
fn rejected_identity_continues_fitted_proof_and_charges_both_grids_for_each_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path());
    let budget = MemoryBudget::new(65536);
    for (index, request) in r.iter().enumerate() {
        let mut bytes = b"PF\n8 8\n-1.0\n".to_vec();
        for _y in (0..8).rev() {
            for x in 0..8 {
                for _ in 0..3 {
                    bytes.extend((-1. - x as f32 + if index == 0 { 0. } else { 0.02 }).to_le_bytes());
                }
            }
        }
        std::fs::write(&request.path, bytes).unwrap();
    }
    let run = |cap| {
        let mut p = policy();
        p.geometry.tolerance = 0.05;
        p.minimum_coverage = 0.8;
        p.pixels.max_source_pixels = cap;
        compare_projected_model_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            &budget,
            |_, a, c, b, _| {
                assert_eq!(&*a.pixels, &*c.pixels);
                let mut points = b.try_buffer(
                    6,
                    ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 1.,
                    },
                )?;
                for (i, q) in points.iter_mut().enumerate() {
                    q.source = [1. + 2. * (i % 3) as f64, 1. + 4. * (i / 3) as f64];
                    q.target = [q.source[0] + 0.02, q.source[1]];
                }
                Ok(points.freeze())
            },
            || false,
        )
    };
    let e = run(256).unwrap();
    assert!(e.supported);
    assert_eq!((e.pixel_attempts, e.pixel_work), (2, 256));
    assert_eq!(e.pixels.unwrap().forward.matched_pixels, 56);
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        run(255),
        Err(ProjectedModelError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
}
