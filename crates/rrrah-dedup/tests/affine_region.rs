#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    affine_color::AffineColorPolicy,
    affine_region::{AffineRegionError, AffineRegionPolicy, verify_affine_region},
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
};
fn fixture() -> Vec<f32> {
    (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| [x as f32 / 128., y as f32 / 128., (x * y) as f32 / 4096., 1.])
        })
        .collect()
}
fn policy() -> AffineRegionPolicy {
    AffineRegionPolicy {
        radius: 1,
        maximum_sites: 256,
        maximum_pixel_reads: 11520,
        color: AffineColorPolicy {
            minimum_samples: 100,
            maximum_samples: 256,
            minimum_variance: 1e-8,
            minimum_relative_pivot: 1e-8,
            maximum_coefficient: 5.,
            maximum_offset: 0.1,
        },
        tolerance: 0.001,
    }
}
fn identity() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
#[test]
fn identity_is_heldout_verified_and_memory_released() {
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let b = MemoryBudget::new(24576);
    let e = verify_affine_region(
        &view,
        &view,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy(),
        &b,
        || false,
    )
    .unwrap();
    assert_eq!(e.training_samples, 128);
    assert_eq!(e.heldout.samples, 128);
    assert_eq!(e.heldout.matched, 128);
    assert_eq!(e.pixel_reads, 11520);
    assert_eq!(b.used(), 0);
    assert_eq!(b.peak(), 24576);
}
#[test]
fn preflight_and_mid_sampling_cancel_release_credit() {
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let b = MemoryBudget::new(24575);
    assert_eq!(
        verify_affine_region(
            &view,
            &view,
            identity(),
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy(),
            &b,
            || false
        ),
        Err(AffineRegionError::Budget)
    );
    assert_eq!(b.used(), 0);
    let b = MemoryBudget::new(24576);
    for stop in [0, 1, 10, 100, 1000] {
        let calls = std::cell::Cell::new(0);
        let result = verify_affine_region(
            &view,
            &view,
            identity(),
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            policy(),
            &b,
            || {
                let n = calls.get();
                calls.set(n + 1);
                n >= stop
            },
        );
        assert_eq!(result, Err(AffineRegionError::Cancelled));
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn saturated_uniform_filter_refuses_as_uninformative_not_invalid() {
    let data = vec![1.0; 64 * 64 * 4];
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let budget = MemoryBudget::new(1048576);
    let mut p = policy();
    p.radius = 8;
    p.maximum_pixel_reads = 256 * 289 * 5;
    let result = verify_affine_region(
        &view,
        &view,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        p,
        &budget,
        || false,
    );
    assert_eq!(
        result,
        Err(AffineRegionError::Color(
            rrrah_dedup::affine_color::AffineColorError::Uninformative
        ))
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn radius24_is_admitted_by_work_budget_and_refused_before_allocation_when_short() {
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let mut p = policy();
    p.radius = 24;
    p.maximum_pixel_reads = 256 * 49 * 49 * 5;
    let budget = MemoryBudget::new(24576);
    let evidence = verify_affine_region(
        &view,
        &view,
        identity(),
        [24, 24, 16, 16],
        [24, 24, 16, 16],
        p,
        &budget,
        || false,
    )
    .unwrap();
    assert!(evidence.heldout.samples >= 100);
    assert_eq!(evidence.heldout.matched, evidence.heldout.samples);
    assert_eq!(budget.used(), 0);
    let short = MemoryBudget::new(24576);
    p.maximum_pixel_reads -= 1;
    assert_eq!(
        verify_affine_region(
            &view,
            &view,
            identity(),
            [24, 24, 16, 16],
            [24, 24, 16, 16],
            p,
            &short,
            || false
        ),
        Err(AffineRegionError::Budget)
    );
    assert_eq!(short.peak(), 0);
}

#[test]
fn source_footprint_preserves_analytic_anisotropic_pair_and_accounts_eight_reads() {
    use rrrah_dedup::affine_region::{AffineRegionFootprint, verify_affine_region_with_footprint};
    let source = fixture();
    let target: Vec<f32> = (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| [x as f32 / 256., y as f32 / 64., (x * y) as f32 / 4096., 1.])
        })
        .collect();
    let av = LinearRgbaView::new(64, 64, &source, 4096, || false).unwrap();
    let bv = LinearRgbaView::new(64, 64, &target, 4096, || false).unwrap();
    let model = ProjectiveTransform {
        matrix: [[2., 0., 0.], [0., 0.5, 0.], [0., 0., 1.]],
    };
    let mut p = policy();
    p.maximum_pixel_reads = 256 * 9 * 8;
    let budget = MemoryBudget::new(24576);
    let result = verify_affine_region_with_footprint(
        &av,
        &bv,
        model,
        [8, 20, 16, 16],
        [16, 10, 32, 8],
        p,
        AffineRegionFootprint::Source,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(result.training_samples, 128);
    assert_eq!(result.heldout.samples, 128);
    assert_eq!(result.heldout.matched, 128);
    assert_eq!(result.pixel_reads, 256 * 9 * 8);
    assert_eq!(budget.used(), 0);
    p.maximum_pixel_reads -= 1;
    let short = MemoryBudget::new(24576);
    assert_eq!(
        verify_affine_region_with_footprint(
            &av,
            &bv,
            model,
            [8, 20, 16, 16],
            [16, 10, 32, 8],
            p,
            AffineRegionFootprint::Source,
            &short,
            || false
        ),
        Err(AffineRegionError::Budget)
    );
    assert_eq!(short.peak(), 0);
}

#[test]
fn source_footprint_mid_tap_cancel_releases_all_managed_credit() {
    use rrrah_dedup::affine_region::{AffineRegionFootprint, verify_affine_region_with_footprint};
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let mut p = policy();
    p.maximum_pixel_reads = 256 * 9 * 8;
    let budget = MemoryBudget::new(24576);
    let calls = std::cell::Cell::new(0);
    let result = verify_affine_region_with_footprint(
        &view,
        &view,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        p,
        AffineRegionFootprint::Source,
        &budget,
        || {
            let n = calls.get();
            calls.set(n + 1);
            n == 50
        },
    );
    assert_eq!(result, Err(AffineRegionError::Cancelled));
    assert_eq!(budget.used(), 0);
    assert_eq!(budget.peak(), 24576);
}

#[test]
fn training_diagnostic_distinguishes_heldout_damage_without_extra_pixels() {
    use rrrah_dedup::affine_region::{AffineRegionFootprint, diagnose_affine_region_with_footprint};
    let data = fixture();
    let mut damaged = data.clone();
    // Only interiors of heldout checker cells change; radius-one training taps
    // never touch these pixels. The training fit must remain the identity.
    for y in 17..23 {
        for x in 25..31 {
            damaged[(y * 64 + x) * 4] += 0.15;
        }
    }
    let source = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let target = LinearRgbaView::new(64, 64, &damaged, 4096, || false).unwrap();
    let budget = MemoryBudget::new(24576);
    let baseline = verify_affine_region(
        &source,
        &target,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    let diagnostic = diagnose_affine_region_with_footprint(
        &source,
        &target,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy(),
        AffineRegionFootprint::Target,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(diagnostic.evidence, baseline);
    assert_eq!(diagnostic.training.samples, 128);
    assert_eq!(diagnostic.training.matched, 128);
    assert!(diagnostic.training.squared_error < 1e-20);
    assert!(diagnostic.evidence.heldout.matched < 128);
    assert!(diagnostic.evidence.heldout.squared_error > 0.01);
    assert_eq!(diagnostic.evidence.pixel_reads, 11520);
    assert_eq!(budget.peak(), 24576);
    assert_eq!(budget.used(), 0);
}

#[test]
fn diagnostic_training_pass_cancels_and_releases_samples() {
    use rrrah_dedup::affine_color::AffineColorError;
    use rrrah_dedup::affine_region::{AffineRegionFootprint, diagnose_affine_region_with_footprint};
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let budget = MemoryBudget::new(24576);
    let calls = std::cell::Cell::new(0);
    verify_affine_region(
        &view,
        &view,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let stop = calls.get() + 10;
    calls.set(0);
    let result = diagnose_affine_region_with_footprint(
        &view,
        &view,
        identity(),
        [16, 16, 16, 16],
        [16, 16, 16, 16],
        policy(),
        AffineRegionFootprint::Target,
        &budget,
        || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        },
    );
    assert_eq!(result, Err(AffineRegionError::Color(AffineColorError::Cancelled)));
    assert_eq!(budget.used(), 0);
}
