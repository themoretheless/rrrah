#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    anchor_rank::{AnchorRankPolicy, compare_anchor_rank},
    geometry::{Correspondence, ProjectiveTransform},
    local_rank::{LocalRankError, LocalRankPolicy},
    rank_region::RankRegionPolicy,
};
use rrrah_dedup::{
    anchor_rank_file::{AnchorRankFileError, compare_anchor_rank_files},
    animated::AnimationBudget,
};
use std::cell::Cell;
fn model() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn points() -> Vec<Correspondence> {
    (0..10)
        .map(|i| {
            let p = [8. + f64::from(i % 5) * 8., 8. + f64::from(i / 5) * 16.];
            Correspondence { source: p, target: p }
        })
        .collect()
}
fn policy() -> AnchorRankPolicy {
    AnchorRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 2,
                minimum_contrast: 0.001,
                minimum_pairs: 100,
                maximum_sites: 980,
                maximum_pixel_reads: 4900,
            },
            filter_radius: 0,
            minimum_witnesses: 10,
            maximum_points: 10,
            maximum_point_checks: 55,
            minimum_point_separation: 2.,
            target_tolerance: 0.01,
            source_tolerance: 0.01,
            minimum_coverage: 0.3,
            minimum_agreement: 0.9,
        },
        window_radius: 1,
        maximum_selection_checks: 145,
    }
}
fn color_bmp() -> Vec<u8> {
    let pixels = 64 * 64 * 3u32;
    let mut data = Vec::new();
    data.extend(b"BM");
    data.extend((54 + pixels).to_le_bytes());
    data.extend([0; 4]);
    data.extend(54u32.to_le_bytes());
    data.extend(40u32.to_le_bytes());
    data.extend(64i32.to_le_bytes());
    data.extend(64i32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(24u16.to_le_bytes());
    data.extend([0; 24]);
    for y in 0..64u32 {
        for x in 0..64u32 {
            data.extend([(x * y / 16) as u8, (y * 3) as u8, (x * 3) as u8]);
        }
    }
    data
}

#[test]
fn files_match_views_and_cancel_or_mutation_discard_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.bmp");
    std::fs::write(&path, color_bmp()).unwrap();
    let request = DecodeRequest::new(&path);
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 4096,
        max_file_bytes: 65536,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let e = compare_anchor_rank_files(
        &request,
        &request,
        model(),
        &points(),
        decode,
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(e.supported);
    {
        let decoded =
            rrrah_dedup::decode::decode_selected_frame_bounded(&request, decode, &budget, &|| false).unwrap();
        let view = decoded.view(&|| false).unwrap();
        let expected =
            compare_anchor_rank(&view, &view, model(), &points(), policy(), &budget, || false).unwrap();
        assert_eq!(e, expected);
    }
    assert_eq!(budget.used(), 0);
    for stop in [1, calls.get() / 2, calls.get()] {
        let count = Cell::new(0);
        assert!(matches!(
            compare_anchor_rank_files(
                &request,
                &request,
                model(),
                &points(),
                decode,
                policy(),
                &budget,
                || {
                    count.set(count.get() + 1);
                    count.get() == stop
                }
            ),
            Err(AnchorRankFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let count = Cell::new(0);
    let changed = Cell::new(false);
    let result = compare_anchor_rank_files(
        &request,
        &request,
        model(),
        &points(),
        decode,
        policy(),
        &budget,
        || {
            count.set(count.get() + 1);
            if count.get() == calls.get() - 1 {
                std::fs::write(&path, b"changed").unwrap();
                changed.set(true);
            }
            false
        },
    );
    assert!(changed.get());
    assert!(result.is_err());
    assert_eq!(budget.used(), 0);
}
#[test]
fn invalid_anchor_policy_refuses_before_missing_source_io() {
    let request = DecodeRequest::new("/nonexistent/rrrah-invalid-anchor-policy.bmp");
    let decode = AnimationBudget { max_frames: 1, max_pixels: 4096, max_file_bytes: 65536 };
    let budget = MemoryBudget::new(1000000);
    for case in 0..10 {
        let mut p = policy();
        match case {
            0 => p.local.minimum_coverage = f64::NAN,
            1 => p.local.minimum_agreement = 1.01,
            2 => p.local.rank.minimum_contrast = 0.,
            3 => p.local.minimum_point_separation = f64::INFINITY,
            4 => p.local.source_tolerance = -1.,
            5 => p.local.target_tolerance = 0.,
            6 => p.local.rank.radius = 0,
            7 => p.local.rank.minimum_pairs = 0,
            8 => p.local.minimum_witnesses = 3,
            _ => p.local.maximum_points = 9,
        }
        assert!(matches!(compare_anchor_rank_files(&request, &request, model(), &points(), decode, p, &budget, || false),
                         Err(AnchorRankFileError::Rank(LocalRankError::Invalid))), "case={case}");
        assert!(matches!(compare_anchor_rank_files(&request, &request, model(), &points(), decode, p, &budget, || true),
                         Err(AnchorRankFileError::Cancelled)), "case={case}");
    }
    assert_eq!(budget.peak(), 0);
}

#[test]
fn aggregate_work_refuses_before_missing_source_io() {
    let request = DecodeRequest::new("/nonexistent/rrrah-anchor-work.bmp");
    let decode = AnimationBudget {
        max_frames: 1,
        max_pixels: 4096,
        max_file_bytes: 65536,
    };
    let budget = MemoryBudget::new(1000000);
    let mut p = policy();
    p.maximum_selection_checks -= 1;
    assert!(matches!(
        compare_anchor_rank_files(&request, &request, model(), &points(), decode, p, &budget, || {
            false
        }),
        Err(AnchorRankFileError::Rank(LocalRankError::Budget))
    ));
    assert_eq!(budget.peak(), 0);
}
