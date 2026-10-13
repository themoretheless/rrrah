#![cfg(feature = "raster")]
use rrrah_dedup::{
    competitive_rank::compare_competitive_rank_region,
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    rank_region::{RankRegionError, RankRegionPolicy},
};
use std::cell::Cell;
fn identity() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn policy() -> RankRegionPolicy {
    RankRegionPolicy {
        radius: 1,
        minimum_contrast: 0.001,
        minimum_pairs: 1,
        maximum_sites: 1024,
        maximum_pixel_reads: 1_000_000,
    }
}
fn image(f: impl Fn(u32, u32) -> f32) -> Vec<f32> {
    (0..32)
        .flat_map(|y| {
            (0..32)
                .flat_map(|x| {
                    let v = f(x, y);
                    [v, v, v, 1.]
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
fn view(data: &[f32]) -> LinearRgbaView<'_> {
    LinearRgbaView::new(32, 32, data, 1024, || false).unwrap()
}
fn random() -> Vec<f32> {
    image(|x, y| {
        let mut n = x + 32 * y + 173;
        n ^= n << 13;
        n ^= n >> 17;
        n ^= n << 5;
        (n % 1000) as f32 / 1000.
    })
}
#[test]
fn smooth_plane_agrees_with_every_competitor_so_has_no_specificity_margin() {
    let data = image(|x, y| 0.1 + x as f32 * 0.003 + y as f32 * 0.007);
    let v = view(&data);
    let e = compare_competitive_rank_region(
        &v,
        &v,
        identity(),
        [10, 10, 4, 4],
        &[[-4, 0], [4, 0], [0, -4], [0, 4]],
        policy(),
        1,
        || false,
    )
    .unwrap();
    assert_eq!(e.valid_sites, 16);
    assert!(e.informative_pairs > 0);
    assert_eq!(e.baseline_agreeing_pairs, e.informative_pairs);
    assert_eq!(e.best_competitor_agreeing_pairs, e.informative_pairs);
    for c in &e.competitors[..4] {
        assert_eq!(c.agreeing_pairs, e.informative_pairs);
    }
}
#[test]
fn textured_identity_beats_translations_on_the_same_source_defined_pairs() {
    let data = random();
    let v = view(&data);
    let offsets = [[-5, 0], [5, 0], [0, -5], [0, 5]];
    let e =
        compare_competitive_rank_region(&v, &v, identity(), [10, 10, 8, 8], &offsets, policy(), 0, || {
            false
        })
        .unwrap();
    assert_eq!(e.valid_sites, 64);
    assert_eq!(e.baseline_agreeing_pairs, e.informative_pairs);
    assert!(e.best_competitor_agreeing_pairs < e.baseline_agreeing_pairs);
    // Independent integer-grid, unfiltered ordinal oracle (no projective sampler).
    let sample = |x: u32, y: u32| f64::from(data[((y * 32 + x) * 4) as usize]);
    let mut informative = 0;
    let mut counts = [0u64; 4];
    for y in 10..18 {
        for x in 10..18 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let a = sample((x as i32 + dx) as u32, (y as i32 + dy) as u32) - sample(x, y);
                    if a.abs() < 0.001 {
                        continue;
                    }
                    informative += 1;
                    for (k, [ox, oy]) in offsets.iter().enumerate() {
                        let b = sample((x as i32 + dx + ox) as u32, (y as i32 + dy + oy) as u32)
                            - sample((x as i32 + ox) as u32, (y as i32 + oy) as u32);
                        counts[k] += u64::from(b.abs() >= 0.001 && (a > 0.) == (b > 0.));
                    }
                }
            }
        }
    }
    assert_eq!(informative, e.informative_pairs);
    for (k, c) in e.competitors[..4].iter().enumerate() {
        assert_eq!(c.agreeing_pairs, counts[k]);
    }
}
#[test]
fn flat_target_pairs_are_disagreement_without_shrinking_denominator() {
    let data = random();
    let flat = image(|_, _| 0.5);
    let a = view(&data);
    let b = view(&flat);
    let e =
        compare_competitive_rank_region(&a, &b, identity(), [10, 10, 4, 4], &[[4, 0]], policy(), 0, || {
            false
        })
        .unwrap();
    assert!(e.informative_pairs > 0);
    assert_eq!(e.baseline_agreeing_pairs, 0);
    assert_eq!(e.competitors[0].agreeing_pairs, 0);
    let e =
        compare_competitive_rank_region(&b, &a, identity(), [10, 10, 4, 4], &[[4, 0]], policy(), 0, || {
            false
        });
    assert_eq!(e, Err(RankRegionError::Uninformative));
}
#[test]
fn common_edges_skip_only_one_site_and_never_publish_partial_support() {
    let data = random();
    let v = view(&data);
    let e =
        compare_competitive_rank_region(&v, &v, identity(), [0, 8, 8, 1], &[[2, 0]], policy(), 0, || false)
            .unwrap();
    assert_eq!(e.sites, 8);
    assert_eq!(e.valid_sites, 7);
    assert_eq!(
        compare_competitive_rank_region(&v, &v, identity(), [0, 0, 1, 1], &[[2, 0]], policy(), 0, || false),
        Err(RankRegionError::Uninformative)
    );
}
#[test]
fn aggregate_budgets_offsets_and_unsupported_competitor_pixels_refuse() {
    let data = random();
    let v = view(&data);
    let mut p = policy();
    p.maximum_sites = 0;
    assert_eq!(
        compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], p, 0, || false),
        Err(RankRegionError::Budget)
    );
    p = policy();
    p.maximum_pixel_reads = 53;
    assert_eq!(
        compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], p, 0, || false),
        Err(RankRegionError::Budget)
    );
    p.maximum_pixel_reads = 54;
    assert_eq!(
        compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], p, 0, || false)
            .unwrap()
            .pixel_reads,
        54
    );
    for offsets in [&[][..], &[[0, 0]][..], &[[4, 0], [4, 0]][..], &[[1, 0]; 17][..]] {
        assert_eq!(
            compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], offsets, policy(), 0, || false),
            Err(RankRegionError::Invalid)
        );
    }
    assert_eq!(
        compare_competitive_rank_region(
            &v,
            &v,
            identity(),
            [8, 8, 1, 1],
            &[[4, 0]],
            policy(),
            u32::MAX,
            || false
        ),
        Err(RankRegionError::Budget)
    );
    for (channel, value) in [(0, 2.), (3, 0.5)] {
        let mut bad = data.clone();
        bad[((8 * 32 + 12) * 4) + channel] = value;
        let b = view(&bad);
        assert_eq!(
            compare_competitive_rank_region(&v, &b, identity(), [8, 8, 1, 1], &[[4, 0]], policy(), 0, || {
                false
            }),
            Err(RankRegionError::UnsupportedPixels)
        );
    }
}
#[test]
fn every_one_shot_cancel_checkpoint_discards_evidence_and_identical_retry_succeeds() {
    let data = random();
    let v = view(&data);
    let count = Cell::new(0);
    let e = compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], policy(), 1, || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    let total = count.get();
    assert!(total > 100);
    for stop in 1..=total {
        count.set(0);
        let r =
            compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], policy(), 1, || {
                count.set(count.get() + 1);
                count.get() == stop
            });
        assert_eq!(r, Err(RankRegionError::Cancelled), "stop={stop}/{total}");
    }
    assert_eq!(
        compare_competitive_rank_region(&v, &v, identity(), [8, 8, 1, 1], &[[4, 0]], policy(), 1, || false)
            .unwrap(),
        e
    );
}
