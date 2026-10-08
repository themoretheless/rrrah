use rrrah_dedup::{
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    rank_region::{RankRegionError, RankRegionPolicy, compare_rank_region},
};
fn model() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn fixture(map: impl Fn(f32) -> f32) -> Vec<f32> {
    (0..32)
        .flat_map(|y| {
            (0..32)
                .flat_map(|x| {
                    let v = map(0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.);
                    [v, v, v, 1.]
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
fn policy() -> RankRegionPolicy {
    RankRegionPolicy {
        radius: 2,
        minimum_contrast: 1e-4,
        minimum_pairs: 1000,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
    }
}
#[test]
fn nonlinear_monotone_transfer_preserves_local_order_but_inversion_does_not() {
    let data = fixture(|v| v);
    let transformed = fixture(|v| v * v);
    let inverted = fixture(|v| 1. - v);
    let source = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    for (samples, agreement) in [(&transformed, true), (&inverted, false)] {
        let target = LinearRgbaView::new(32, 32, samples, 1024, || false).unwrap();
        let result = compare_rank_region(
            &source,
            &target,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            policy(),
            || false,
        )
        .unwrap();
        assert_eq!(result.sites, 256);
        assert_eq!(result.valid_sites, 256);
        assert_eq!(result.pixel_reads, 11520);
        assert!(result.informative_pairs >= 1000);
        assert_eq!(
            result.agreeing_pairs,
            if agreement { result.informative_pairs } else { 0 }
        );
    }
}
#[test]
fn flat_pixels_budget_and_cancellation_do_not_produce_evidence() {
    let flat = vec![0.5; 32 * 32 * 4];
    let mut flat = flat;
    for p in flat.chunks_exact_mut(4) {
        p[3] = 1.;
    }
    let view = LinearRgbaView::new(32, 32, &flat, 1024, || false).unwrap();
    assert_eq!(
        compare_rank_region(
            &view,
            &view,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            policy(),
            || false
        ),
        Err(RankRegionError::Uninformative)
    );
    let mut short = policy();
    short.maximum_pixel_reads -= 1;
    assert_eq!(
        compare_rank_region(
            &view,
            &view,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            short,
            || false
        ),
        Err(RankRegionError::Budget)
    );
    for stop in [0, 1, 10, 100] {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            compare_rank_region(
                &view,
                &view,
                model(),
                [0, 0, 32, 32],
                [8, 8, 16, 16],
                policy(),
                || {
                    let n = calls.get();
                    calls.set(n + 1);
                    n >= stop
                }
            ),
            Err(RankRegionError::Cancelled)
        );
    }
}
#[test]
fn invalid_domains_policy_alpha_and_scene_range_are_explicit() {
    let data = fixture(|v| v);
    let source = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    assert_eq!(
        compare_rank_region(
            &source,
            &source,
            model(),
            [u32::MAX, 0, 2, 2],
            [8, 8, 16, 16],
            policy(),
            || false
        ),
        Err(RankRegionError::Invalid)
    );
    let mut bad = policy();
    bad.minimum_contrast = 0.;
    assert_eq!(
        compare_rank_region(
            &source,
            &source,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            bad,
            || false
        ),
        Err(RankRegionError::Invalid)
    );
    for (channel, value) in [(3, 0.5), (0, 2.)] {
        let mut changed = data.clone();
        for p in changed.chunks_exact_mut(4) {
            p[channel] = value;
        }
        let target = LinearRgbaView::new(32, 32, &changed, 1024, || false).unwrap();
        assert_eq!(
            compare_rank_region(
                &source,
                &target,
                model(),
                [0, 0, 32, 32],
                [8, 8, 16, 16],
                policy(),
                || false
            ),
            Err(RankRegionError::Invalid)
        );
    }
}

#[test]
fn independent_textures_are_not_high_agreement_evidence() {
    fn noise(seed: u32) -> Vec<f32> {
        let mut state = seed;
        let mut out = Vec::new();
        for _ in 0..1024 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let v = 0.1 + (state % 1000) as f32 / 1250.;
            out.extend_from_slice(&[v, v, v, 1.]);
        }
        out
    }
    let a = noise(0x12345678);
    let b = noise(0x87654321);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let bv = LinearRgbaView::new(32, 32, &b, 1024, || false).unwrap();
    let result = compare_rank_region(
        &av,
        &bv,
        model(),
        [0, 0, 32, 32],
        [8, 8, 16, 16],
        policy(),
        || false,
    )
    .unwrap();
    assert!(result.informative_pairs >= 1000);
    assert!(result.agreeing_pairs * 10 < result.informative_pairs * 7);
}

#[test]
fn filtered_identity_matches_independent_box_counts_and_read_budget() {
    use rrrah_dedup::rank_region::compare_filtered_rank_region;
    let data = fixture(|v| v);
    let view = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let mut p = policy();
    p.maximum_pixel_reads *= 49;
    let e = compare_filtered_rank_region(
        &view,
        &view,
        model(),
        [0, 0, 32, 32],
        [8, 8, 16, 16],
        p,
        3,
        || false,
    )
    .unwrap();
    let mean = |x: usize, y: usize| {
        let mut sum = 0.;
        for py in y - 3..=y + 3 {
            for px in x - 3..=x + 3 {
                sum += f64::from(data[(py * 32 + px) * 4]);
            }
        }
        sum / 49.
    };
    let mut expected = 0;
    for y in 8..24 {
        for x in 8..24 {
            let center = mean(x, y);
            for (dx, dy) in [
                (-2, -2),
                (0, -2),
                (2, -2),
                (-2, 0),
                (2, 0),
                (-2, 2),
                (0, 2),
                (2, 2),
            ] {
                if (mean((x as isize + dx) as usize, (y as isize + dy) as usize) - center).abs()
                    >= p.minimum_contrast
                {
                    expected += 1;
                }
            }
        }
    }
    assert_eq!(e.valid_sites, 256);
    assert_eq!(e.pixel_reads, 564480);
    assert_eq!(e.informative_pairs, expected);
    assert_eq!(e.agreeing_pairs, expected);
    p.maximum_pixel_reads -= 1;
    assert_eq!(
        compare_filtered_rank_region(
            &view,
            &view,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            p,
            3,
            || false
        ),
        Err(RankRegionError::Budget)
    );
}

#[test]
fn filter_zero_preserves_evidence_and_callback_work() {
    use rrrah_dedup::rank_region::compare_filtered_rank_region;
    let data = fixture(|v| v);
    let view = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let calls = std::cell::Cell::new(0);
    let a = compare_rank_region(
        &view,
        &view,
        model(),
        [0, 0, 32, 32],
        [8, 8, 16, 16],
        policy(),
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let total = calls.get();
    calls.set(0);
    let b = compare_filtered_rank_region(
        &view,
        &view,
        model(),
        [0, 0, 32, 32],
        [8, 8, 16, 16],
        policy(),
        0,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(total, calls.get());
}

#[test]
fn filtered_overflow_and_mid_window_cancel_return_no_partial_evidence() {
    use rrrah_dedup::rank_region::compare_filtered_rank_region;
    let data = fixture(|v| v);
    let view = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let mut p = policy();
    p.maximum_pixel_reads = u64::MAX;
    assert_eq!(
        compare_filtered_rank_region(
            &view,
            &view,
            model(),
            [0, 0, 32, 32],
            [8, 8, 16, 16],
            p,
            u32::MAX,
            || false
        ),
        Err(RankRegionError::Budget)
    );
    for stop in [4, 10, 50] {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            compare_filtered_rank_region(
                &view,
                &view,
                model(),
                [0, 0, 32, 32],
                [8, 8, 16, 16],
                p,
                3,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }
            ),
            Err(RankRegionError::Cancelled)
        );
    }
}
