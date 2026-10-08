#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    affine_color::AffineColorPolicy,
    affine_region::{AffineRegionError, AffineRegionPolicy, verify_affine_region},
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
};
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
        tolerance: 0.03,
    }
}
fn identity() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
#[test]
fn unsupported_alpha_and_scene_range_refuse_in_either_direction_without_clipping() {
    let good = [0.2, 0.3, 0.4, 1.0].repeat(4096);
    let normal = LinearRgbaView::new(64, 64, &good, 4096, || false).unwrap();
    for pixel in [
        [0.2, 0.3, 0.4, 0.5],
        [1.5, 0.3, 0.4, 1.0],
        [-0.25, 0.3, 0.4, 1.0],
        [0.2, 0.3, 0.4, 0.0],
    ] {
        let data = pixel.repeat(4096);
        let unsupported = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
        let budget = MemoryBudget::new(1048576);
        for (source, target) in [(&unsupported, &normal), (&normal, &unsupported)] {
            assert_eq!(
                verify_affine_region(
                    source,
                    target,
                    identity(),
                    [16, 16, 16, 16],
                    [16, 16, 16, 16],
                    policy(),
                    &budget,
                    || false
                ),
                Err(AffineRegionError::Invalid)
            );
            assert_eq!(budget.used(), 0);
        }
    }
}
#[test]
fn overflowed_domains_and_radius_refuse_before_allocation() {
    let data = [0.2, 0.3, 0.4, 1.0].repeat(4096);
    let view = LinearRgbaView::new(64, 64, &data, 4096, || false).unwrap();
    let budget = MemoryBudget::new(1048576);
    for domain in [
        [u32::MAX, 0, 16, 16],
        [0, u32::MAX, 16, 16],
        [0, 0, 0, 16],
        [60, 60, 16, 16],
    ] {
        assert_eq!(
            verify_affine_region(
                &view,
                &view,
                identity(),
                domain,
                [16, 16, 16, 16],
                policy(),
                &budget,
                || false
            ),
            Err(AffineRegionError::Invalid)
        );
    }
    let mut p = policy();
    p.radius = u32::MAX;
    assert_eq!(
        verify_affine_region(
            &view,
            &view,
            identity(),
            [16, 16, 16, 16],
            [16, 16, 16, 16],
            p,
            &budget,
            || false
        ),
        Err(AffineRegionError::Budget)
    );
    assert_eq!(budget.peak(), 0);
    assert_eq!(budget.used(), 0);
}
