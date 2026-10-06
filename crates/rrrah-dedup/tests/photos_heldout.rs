#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{LocalFilePolicy, compare_local_files_filtered},
    warp::{FilterPolicy, PhotometricPolicy, WarpPolicy},
};
use std::path::PathBuf;
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 200_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 200_000,
            max_candidates: 200_000,
            max_features: 500,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 250_000,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 500,
            max_hypotheses: 125_000,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 200_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    }
}
fn photometric_policy() -> PhotometricPolicy {
    PhotometricPolicy {
        residual: policy().pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    }
}

const IDS: [u32; 6] = [2414, 2418, 2883, 5025, 1425, 5495];
fn request(id: u32, variant: &str) -> DecodeRequest {
    let suffix = if variant == "jpeg" { "jpg" } else { "png" };
    DecodeRequest::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/photos-heldout")
            .join(format!("{id}-{variant}.{suffix}")),
    )
}
#[test]
fn exact_pixels_keep_related_captures_and_other_scenes_distinct() {
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let frames: Vec<_> = IDS
        .iter()
        .map(|id| {
            rrrah_dedup::decode::decode_selected_frame(&request(*id, "base"), 200_000, &budget, || false)
                .unwrap()
        })
        .collect();
    for (i, frame) in frames.iter().enumerate() {
        assert!(frame.same_selected_frame(frame, || false).unwrap());
        for (j, other) in frames.iter().enumerate().skip(i + 1) {
            assert!(
                !frame.same_selected_frame(other, || false).unwrap(),
                "{} vs {}",
                IDS[i],
                IDS[j]
            );
        }
    }
    drop(frames);
    assert_eq!(budget.used(), 0);
}
#[test]
fn unchanged_filter_policy_accepts_heldout_derivatives() {
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut failures = Vec::new();
    for id in IDS {
        for variant in ["resize", "crop", "rotate", "brightness", "jpeg"] {
            let result = compare_local_files_filtered(
                &request(id, "base"),
                &request(id, variant),
                policy(),
                photometric_policy(),
                FilterPolicy {
                    radius: 2,
                    max_sample_pairs: 20_000_000,
                },
                &budget,
                || false,
            );
            match result {
                Ok(e) if e.candidate => eprintln!("heldout positive {id} {variant}: accepted"),
                other => failures.push(format!("{id} {variant}: {other:?}")),
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert!(
        failures.is_empty(),
        "heldout misses with unchanged policy: {failures:#?}"
    );
}
#[test]
fn unchanged_filter_policy_rejects_unrelated_heldout_scenes() {
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for (i, a) in IDS.iter().enumerate() {
        for b in IDS.iter().skip(i + 1) {
            // These two sources show related views of the same garden.
            if (*a, *b) == (2414, 2418) {
                continue;
            }
            let result = compare_local_files_filtered(
                &request(*a, "base"),
                &request(*b, "base"),
                policy(),
                photometric_policy(),
                FilterPolicy {
                    radius: 2,
                    max_sample_pairs: 20_000_000,
                },
                &budget,
                || false,
            )
            .unwrap();
            assert!(!result.candidate, "unrelated {a} vs {b}: {result:?}");
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn related_garden_views_keep_exact_pixel_evidence_separate_from_visual_candidate() {
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let a = request(2414, "base");
    let b = request(2418, "base");
    let result = compare_local_files_filtered(
        &a,
        &b,
        policy(),
        photometric_policy(),
        FilterPolicy {
            radius: 2,
            max_sample_pairs: 20_000_000,
        },
        &budget,
        || false,
    )
    .unwrap();
    eprintln!("related garden views: {result:?}");
    if result.candidate {
        assert!(result.geometry.is_some() && result.pixels.is_some() && result.filtered.is_some());
    }
    if let Some(pixels) = result.pixels {
        assert!(pixels.forward.maximum_channel_error > 0.0 || pixels.reverse.maximum_channel_error > 0.0);
    }
    assert_eq!(budget.used(), 0);
    let left = rrrah_dedup::decode::decode_selected_frame(&a, 200_000, &budget, || false).unwrap();
    let right = rrrah_dedup::decode::decode_selected_frame(&b, 200_000, &budget, || false).unwrap();
    assert!(!left.same_selected_frame(&right, || false).unwrap());
    drop((left, right));
    assert_eq!(budget.used(), 0);
}

#[test]
fn heldout_indexed_crop_collection_matches_complete_pair_oracle() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy, scan_local_files_with_policy},
        warp::{ColorFilterPolicy, FilterColorSpace, PhotometricFitMode},
    };
    let files = IDS
        .into_iter()
        .enumerate()
        .flat_map(|(i, id)| {
            ["base", "crop"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| ((i * 2 + j) as u64, request(id, variant)))
        })
        .collect::<Vec<_>>();
    let search = LocalSearchPolicy {
        local: policy(),
        comparison: LocalComparisonMode::Filtered {
            photometric: photometric_policy(),
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 2,
                    max_sample_pairs: 20_000_000,
                },
                color_space: FilterColorSpace::LinearSrgb,
            },
            fit: PhotometricFitMode::RejectOutsidePolicy,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let reference = scan_local_files_with_policy(files.clone(), search, 12, 66, &budget, || false).unwrap();
    let indexed = scan_local_collection(
        files,
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 12,
                max_features: 6000,
                max_hits: 36_000_000,
                max_pair_counts: 66,
                max_pairs: 66,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    let edges = |r: &rrrah_dedup::local_scan::LocalFileReport| {
        r.pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>()
    };
    let expected = edges(&reference);
    assert_eq!(edges(&indexed.local), expected);
    for i in 0..6 {
        assert!(expected.contains(&(i * 2, i * 2 + 1)), "missing heldout crop {i}");
    }
    for (a, b) in &expected {
        assert!(
            a / 2 == b / 2 || (*a < 4 && *b < 4),
            "unrelated scene edge {a} {b}"
        );
    }
    assert!(reference.issues.is_empty() && reference.source_issues.is_empty());
    assert!(
        indexed.file_issues.is_empty()
            && indexed.local.issues.is_empty()
            && indexed.local.source_issues.is_empty()
    );
    assert_eq!(indexed.analysed.len(), 12);
    assert_eq!(budget.used(), 0);
    eprintln!(
        "heldout collection: accepted={expected:?}, proposed={}, verified={}",
        indexed.proposed_pairs, indexed.pixel_verification_pairs
    );
}
