#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    affine_color::AffineColorPolicy,
    affine_region::AffineRegionPolicy,
    affine_region_grid::{AffineGridPolicy, verify_affine_region_grid},
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
fn automatic_grid_retains_both_direction_results_and_managed_ownership() {
    let data = fixture();
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let budget = MemoryBudget::new(1048576);
    let mut region = policy();
    region.maximum_sites = 1024;
    region.color.maximum_samples = 1024;
    region.maximum_pixel_reads = 46080;
    let p = AffineGridPolicy {
        region,
        grid: (2, 2),
        maximum_regions: 4,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 368640,
    };
    let e = verify_affine_region_grid(&view, &view, identity(), p, &budget, || false).unwrap();
    assert_eq!(e.len(), 4);
    for row in e.iter() {
        assert_eq!(row.source_radius, 1);
        for result in row.directions {
            let value = result.unwrap();
            assert_eq!(value.heldout.matched, value.heldout.samples);
        }
    }
    assert!(budget.used() > 0);
    let clone = e.clone();
    drop(e);
    assert!(budget.used() > 0);
    drop(clone);
    assert_eq!(budget.used(), 0);
    let mut denied = p;
    denied.maximum_total_pixel_reads = 1;
    assert!(verify_affine_region_grid(&view, &view, identity(), denied, &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
}
#[test]
fn uniform_regions_are_retained_as_refusals_and_cancel_discards_partial_grid() {
    let data = vec![0.5, 0.5, 0.5, 1.0].repeat(4096);
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let budget = MemoryBudget::new(1048576);
    let mut region = policy();
    region.maximum_sites = 1024;
    region.color.maximum_samples = 1024;
    region.maximum_pixel_reads = 46080;
    let p = AffineGridPolicy {
        region,
        grid: (2, 2),
        maximum_regions: 4,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 368640,
    };
    let e = verify_affine_region_grid(&view, &view, identity(), p, &budget, || false).unwrap();
    assert_eq!(e.len(), 4);
    for row in e.iter() {
        for result in row.directions {
            assert!(matches!(
                result,
                Err(rrrah_dedup::affine_region::AffineRegionError::Color(
                    rrrah_dedup::affine_color::AffineColorError::Uninformative
                ))
            ));
        }
    }
    drop(e);
    assert_eq!(budget.used(), 0);
    for stop in [0, 10, 1000, 20000] {
        let calls = std::cell::Cell::new(0);
        let result = verify_affine_region_grid(&view, &view, identity(), p, &budget, || {
            let n = calls.get();
            calls.set(n + 1);
            n == stop
        });
        assert!(result.is_err());
        assert_eq!(budget.used(), 0);
    }
}
