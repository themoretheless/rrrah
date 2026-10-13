#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    anchor_rank_file::{AnchorRankFileError, NativePixelRefusal},
    geometry::ProjectiveTransform,
    gradient::{describe_gradient, gradient_orientation},
    gradient_scale::smooth_gradient_candidates_managed,
    linear::LinearRgbaView,
    local::LocalError,
    local_rank::LocalRankError,
    local_scan::LocalFileError,
    rank_region::{RankRegionError, RankRegionPolicy, compare_rank_region},
};
#[test]
fn only_explicit_pixel_domain_errors_can_admit_an_alternate_verifier() {
    for (error, expected) in [
        (
            AnchorRankFileError::Search(LocalFileError::Features(LocalError::UnsupportedPixels)),
            NativePixelRefusal::GradientOpacity,
        ),
        (
            AnchorRankFileError::Rank(LocalRankError::Rank(RankRegionError::UnsupportedPixels)),
            NativePixelRefusal::RankPixelRange,
        ),
        (
            AnchorRankFileError::Search(LocalFileError::Rank(LocalRankError::Rank(
                RankRegionError::UnsupportedPixels,
            ))),
            NativePixelRefusal::RankPixelRange,
        ),
    ] {
        assert_eq!(error.pixel_refusal(), Some(expected));
    }
    for error in [
        AnchorRankFileError::Cancelled,
        AnchorRankFileError::Rank(LocalRankError::Invalid),
        AnchorRankFileError::Rank(LocalRankError::Budget),
        AnchorRankFileError::Rank(LocalRankError::Cancelled),
        AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry),
        AnchorRankFileError::Rank(LocalRankError::Rank(RankRegionError::Invalid)),
        AnchorRankFileError::Rank(LocalRankError::Rank(RankRegionError::Budget)),
        AnchorRankFileError::Rank(LocalRankError::Rank(RankRegionError::Cancelled)),
        AnchorRankFileError::Rank(LocalRankError::Rank(RankRegionError::Uninformative)),
        AnchorRankFileError::Search(LocalFileError::InvalidPolicy),
        AnchorRankFileError::Search(LocalFileError::Features(LocalError::Invalid)),
        AnchorRankFileError::Search(LocalFileError::Features(LocalError::Budget)),
        AnchorRankFileError::Search(LocalFileError::Features(LocalError::Cancelled)),
    ] {
        assert_eq!(error.pixel_refusal(), None, "{error:?}");
    }
}
#[test]
fn gradient_opacity_is_distinct_from_coordinates_work_and_cancellation() {
    let mut pixels = vec![0.2; 48 * 48 * 4];
    for p in pixels.chunks_exact_mut(4) {
        p[3] = 1.;
    }
    pixels[(24 * 48 + 24) * 4 + 3] = 0.5;
    let image = LinearRgbaView::new(48, 48, &pixels, 2304, || false).unwrap();
    assert_eq!(
        gradient_orientation(&image, [24, 24], 256, || false),
        Err(LocalError::UnsupportedPixels)
    );
    assert!(matches!(
        describe_gradient(&image, [24., 24.], 1., 0., 256, || false),
        Err(LocalError::UnsupportedPixels)
    ));
    assert_eq!(
        gradient_orientation(&image, [0, 0], 256, || false),
        Err(LocalError::Invalid)
    );
    assert_eq!(
        gradient_orientation(&image, [24, 24], 1, || false),
        Err(LocalError::Budget)
    );
    assert_eq!(
        gradient_orientation(&image, [24, 24], 256, || true),
        Err(LocalError::Cancelled)
    );
    let budget = MemoryBudget::new(1024 * 1024);
    for radius in [0, 1] {
        assert!(matches!(
            smooth_gradient_candidates_managed(&image, radius, 100000, &budget, || false),
            Err(LocalError::UnsupportedPixels)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn rank_alpha_negative_and_hdr_refusals_preserve_invalid_and_cancelled_errors() {
    let mut pixels = vec![0.2; 32 * 32 * 4];
    for p in pixels.chunks_exact_mut(4) {
        p[3] = 1.;
    }
    let source = LinearRgbaView::new(32, 32, &pixels, 1024, || false).unwrap();
    let policy = RankRegionPolicy {
        radius: 1,
        minimum_contrast: 0.001,
        minimum_pairs: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 20000,
    };
    let transform = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    for (channel, value) in [(3, 0.), (3, 0.5), (0, -0.1), (0, 2.)] {
        let mut changed = pixels.clone();
        for p in changed.chunks_exact_mut(4) {
            p[channel] = value;
        }
        let target = LinearRgbaView::new(32, 32, &changed, 1024, || false).unwrap();
        assert_eq!(
            compare_rank_region(
                &source,
                &target,
                transform,
                [8, 8, 8, 8],
                [8, 8, 8, 8],
                policy,
                || false
            ),
            Err(RankRegionError::UnsupportedPixels)
        );
        assert_eq!(
            compare_rank_region(
                &source,
                &target,
                transform,
                [u32::MAX, 8, 8, 8],
                [8, 8, 8, 8],
                policy,
                || false
            ),
            Err(RankRegionError::Invalid)
        );
        assert_eq!(
            compare_rank_region(
                &source,
                &target,
                transform,
                [8, 8, 8, 8],
                [8, 8, 8, 8],
                policy,
                || true
            ),
            Err(RankRegionError::Cancelled)
        );
    }
}
