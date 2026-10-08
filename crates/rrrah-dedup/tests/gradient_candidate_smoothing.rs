#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    gradient_scale::smooth_gradient_candidates_managed, linear::LinearRgbaView, local::LocalError,
};
fn samples() -> Vec<f32> {
    (0..6)
        .flat_map(|i| {
            let v = i as f32 * 0.37 - 0.5;
            [v, v * 2., v + 1., 1.]
        })
        .collect()
}
#[test]
fn smoothing_matches_independent_footprints_and_exact_last_owner_credit() {
    let pixels = samples();
    let view = LinearRgbaView::new(3, 2, &pixels, 6, || false).unwrap();
    for radius in [0, 1, 2, 4] {
        let side = 2 * radius + 1;
        let budget = MemoryBudget::new(192);
        let output =
            smooth_gradient_candidates_managed(&view, radius, 12 * u64::from(side), &budget, || false)
                .unwrap();
        for y in 0..2i64 {
            for x in 0..3i64 {
                for c in 0..4 {
                    let mut expected = 0.;
                    for dy in -i64::from(radius)..=i64::from(radius) {
                        for dx in -i64::from(radius)..=i64::from(radius) {
                            let sx = (x + dx).clamp(0, 2) as usize;
                            let sy = (y + dy).clamp(0, 1) as usize;
                            expected += f64::from(pixels[(sy * 3 + sx) * 4 + c]) / f64::from(side * side);
                        }
                    }
                    assert!((f64::from(output[((y * 3 + x) * 4) as usize + c]) - expected).abs() < 0.000001);
                }
            }
        }
        if radius == 0 {
            assert_eq!(&*output, &pixels);
        }
        assert_eq!(budget.peak(), if radius == 0 {96} else {192});
        assert_eq!(budget.used(), 96);
        let clone = output.clone();
        drop(output);
        assert_eq!(budget.used(), 96);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn work_memory_opacity_and_every_cancellation_refuse_without_retained_output() {
    let pixels = samples();
    let view = LinearRgbaView::new(3, 2, &pixels, 6, || false).unwrap();
    let budget = MemoryBudget::new(192);
    assert!(matches!(
        smooth_gradient_candidates_managed(&view, 2, 59, &budget, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
    let denied = MemoryBudget::new(191);
    assert!(matches!(
        smooth_gradient_candidates_managed(&view, 2, 60, &denied, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(denied.used(), 0);
    let calls = std::cell::Cell::new(0);
    drop(
        smooth_gradient_candidates_managed(&view, 2, 60, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let call = std::cell::Cell::new(0);
        let b = MemoryBudget::new(192);
        assert!(matches!(
            smooth_gradient_candidates_managed(&view, 2, 60, &b, || {
                call.set(call.get() + 1);
                call.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(b.used(), 0);
    }
    let mut alpha = pixels.clone();
    alpha[3] = 0.5;
    let alpha = LinearRgbaView::new(3, 2, &alpha, 6, || false).unwrap();
    assert!(matches!(
        smooth_gradient_candidates_managed(&alpha, 2, 60, &budget, || false),
        Err(LocalError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        smooth_gradient_candidates_managed(&view, u32::MAX, u64::MAX, &budget, || true),
        Err(LocalError::Cancelled)
    ));
    drop(smooth_gradient_candidates_managed(&view, 2, 60, &budget, || false).unwrap());
    assert_eq!(budget.used(), 0);
}

#[test]
fn zero_radius_exact_bits_half_memory_and_all_cancellation_points() {
    let pixels = [-0.0, f32::MAX, f32::MIN_POSITIVE, 1., -7., 0., f32::from_bits(1), 1.];
    let view = LinearRgbaView::new(2, 1, &pixels, 2, || false).unwrap();
    let budget = MemoryBudget::new(32);
    let calls = std::cell::Cell::new(0);
    let output = smooth_gradient_candidates_managed(&view, 0, 4, &budget, || {calls.set(calls.get()+1); false}).unwrap();
    // Independent two separable one-tap passes, retaining their rounding.
    let expected = pixels.map(|v| {
        let horizontal = (0.0_f64 + f64::from(v)) as f32;
        (0.0_f64 + f64::from(horizontal)) as f32
    });
    assert_eq!(output.iter().map(|v|v.to_bits()).collect::<Vec<_>>(), expected.map(f32::to_bits));
    assert_eq!(budget.peak(), 32); drop(output); assert_eq!(budget.used(), 0);
    for stop in 1..=calls.get() {
        let seen = std::cell::Cell::new(0);
        let b = MemoryBudget::new(32);
        assert!(matches!(smooth_gradient_candidates_managed(&view,0,4,&b,|| {seen.set(seen.get()+1);seen.get()==stop}),Err(LocalError::Cancelled)));
        assert_eq!(b.used(),0);
    }
    for (bytes,taps) in [(31,4),(32,3)] {
        let b=MemoryBudget::new(bytes);
        assert!(matches!(smooth_gradient_candidates_managed(&view,0,taps,&b,||false),Err(LocalError::Budget)));
        assert_eq!(b.used(),0);
    }
    let mut transparent=pixels;transparent[3]=0.5;
    let view=LinearRgbaView::new(2,1,&transparent,2,||false).unwrap();
    assert!(matches!(smooth_gradient_candidates_managed(&view,0,4,&budget,||false),Err(LocalError::Invalid)));
    assert_eq!(budget.used(),0);
}
