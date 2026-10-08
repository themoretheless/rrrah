use rrrah_dedup::{
    gradient::{
        GradientDescriptor, GradientFeature, GradientMatchPolicy, match_gradients,
        match_gradients_distinct_locations,
    },
    local::LocalError,
};
fn f(x: f64, bin: usize) -> GradientFeature {
    let mut d = [0.; 128];
    d[bin] = 1.;
    GradientFeature {
        position: [x, 0.],
        descriptor: GradientDescriptor(d),
    }
}
fn policy(work: u64) -> GradientMatchPolicy {
    GradientMatchPolicy {
        max_comparisons: work,
        max_squared_distance: 0.5,
        squared_ratio: 0.64,
    }
}
#[test]
fn nearby_scale_duplicates_preserve_distinct_landmarks_and_reject_ambiguity() {
    let a = [f(0., 0), f(10., 1)];
    let b = [f(100., 0), f(100.5, 0), f(200., 1)];
    assert_eq!(match_gradients(&a, &b, policy(6), || false).unwrap().len(), 1);
    let result = match_gradients_distinct_locations(&a, &b, policy(12), 2., || false).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].target, [100., 0.]);
    assert_eq!(result[1].target, [200., 0.]);
    let repeated = [f(100., 0), f(200., 0)];
    assert!(
        match_gradients_distinct_locations(&a, &repeated, policy(8), 2., || false)
            .unwrap()
            .is_empty()
    );
    let cluster = [f(100., 0), f(100.5, 1)];
    assert!(
        match_gradients_distinct_locations(&a, &cluster, policy(8), 2., || false)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn distinct_location_work_validation_and_atomic_cancellation() {
    let a = [f(0., 0), f(10., 1)];
    let b = [f(100., 0), f(100.5, 0), f(200., 1)];
    assert!(matches!(
        match_gradients_distinct_locations(&a, &b, policy(11), 2., || false),
        Err(LocalError::Budget)
    ));
    for radius in [-1., f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(matches!(
            match_gradients_distinct_locations(&a, &b, policy(12), radius, || false),
            Err(LocalError::Invalid)
        ));
    }
    let calls = std::cell::Cell::new(0);
    let baseline = match_gradients_distinct_locations(&a, &b, policy(12), 2., || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(
            matches!(
                match_gradients_distinct_locations(&a, &b, policy(12), 2., || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(LocalError::Cancelled)
            ),
            "stop={stop}"
        );
    }
    assert_eq!(
        match_gradients_distinct_locations(&a, &b, policy(12), 2., || false)
            .unwrap()
            .len(),
        baseline.len()
    );
    for (left, right) in [(&a[..], &[][..]), (&[][..], &b[..])] {
        assert!(
            match_gradients_distinct_locations(left, right, policy(0), 2., || false)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            match_gradients_distinct_locations(left, right, policy(0), 2., || true),
            Err(LocalError::Cancelled)
        ));
    }
}
#[test]
fn distinct_location_results_equal_independent_sorted_distance_oracle() {
    let mut seed = 0x12345678u64;
    let mut features = |count: usize| -> Vec<GradientFeature> {
        (0..count)
            .map(|i| {
                let mut d = [0.; 128];
                for v in &mut d {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    *v = ((seed >> 32) % 1000 + 1) as f64;
                }
                let norm = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                for v in &mut d {
                    *v /= norm;
                }
                GradientFeature {
                    position: [(i / 2) as f64 * 10., (i % 2) as f64 * 0.5],
                    descriptor: GradientDescriptor(d),
                }
            })
            .collect()
    };
    let a = features(13);
    let mut b = features(17);
    for i in 0..6 {
        b[i].descriptor = a[i].descriptor;
    }
    let nearest = |source: &[GradientFeature], target: &[GradientFeature], radius: f64| {
        source
            .iter()
            .map(|f| {
                let mut sorted: Vec<_> = target
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        (
                            f.descriptor
                                .0
                                .iter()
                                .zip(t.descriptor.0)
                                .map(|(a, b)| (a - b) * (a - b))
                                .sum::<f64>(),
                            i,
                        )
                    })
                    .collect();
                sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                let best = sorted[0];
                let pos = target[best.1].position;
                let second = sorted
                    .iter()
                    .find(|(_, i)| {
                        let p = target[*i].position;
                        (p[0] - pos[0]).powi(2) + (p[1] - pos[1]).powi(2) > radius * radius
                    })
                    .map(|v| v.0)
                    .unwrap_or(f64::INFINITY);
                (best.0, second, best.1)
            })
            .collect::<Vec<_>>()
    };
    for radius in [0., 2., 12., 1000.] {
        let forward = nearest(&a, &b, radius);
        let reverse = nearest(&b, &a, radius);
        let expected: Vec<_> = forward
            .iter()
            .enumerate()
            .filter_map(|(i, &(d, s, j))| {
                let (rd, rs, ri) = reverse[j];
                (s.is_finite() && rs.is_finite() && d <= 0.5 && d < 0.64 * s && ri == i && rd < 0.64 * rs)
                    .then_some((a[i].position, b[j].position))
            })
            .collect();
        let measured =
            match_gradients_distinct_locations(&a, &b, policy(2 * 13 * 17), radius, || false).unwrap();
        assert_eq!(
            measured.iter().map(|p| (p.source, p.target)).collect::<Vec<_>>(),
            expected,
            "radius={radius}"
        );
    }
}
#[test]
fn managed_distinct_match_admission_last_owner_and_cancellation() {
    use rrrah_dedup::gradient::match_gradients_distinct_locations_managed as managed;
    let a = [f(0., 0), f(10., 1)];
    let b = [f(100., 0), f(100.5, 0), f(200., 1)];
    let calls = std::cell::Cell::new(0);
    let budget = rrrah_core::MemoryBudget::new(4096);
    let result = managed(&a, &b, policy(12), 2., &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    let peak = budget.peak();
    assert_eq!(result.len(), 2);
    let expected = match_gradients_distinct_locations(&a, &b, policy(12), 2., || false).unwrap();
    assert_eq!(
        result.iter().map(|p| (p.source, p.target)).collect::<Vec<_>>(),
        expected.iter().map(|p| (p.source, p.target)).collect::<Vec<_>>()
    );
    let retained = budget.used();
    assert!(retained > 0);
    let clone = result.clone();
    drop(result);
    assert_eq!(budget.used(), retained);
    drop(clone);
    assert_eq!(budget.used(), 0);
    let exact = rrrah_core::MemoryBudget::new(peak);
    drop(managed(&a, &b, policy(12), 2., &exact, || false).unwrap());
    assert_eq!(exact.used(), 0);
    let short = rrrah_core::MemoryBudget::new(peak - 1);
    assert!(matches!(
        managed(&a, &b, policy(12), 2., &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
    for stop in 1..=total {
        calls.set(0);
        let budget = rrrah_core::MemoryBudget::new(4096);
        assert!(matches!(
            managed(&a, &b, policy(12), 2., &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        if stop == 1 {
            assert_eq!(budget.peak(), 0);
        }
    }
    let budget = rrrah_core::MemoryBudget::new(4096);
    assert!(matches!(
        managed(&a, &b, policy(11), 2., &budget, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
}
