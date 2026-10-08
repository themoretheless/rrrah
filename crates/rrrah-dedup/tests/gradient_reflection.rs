#![cfg(feature = "raster")]
use rrrah_dedup::{
    gradient::{describe_gradient, describe_gradient_interpolated, reflect_gradient_descriptor},
    linear::LinearRgbaView,
    local::LocalError,
};
use std::cell::Cell;
fn never() -> bool { false }

#[test]
fn canonical_reflection_matches_independently_mirrored_pixels() {
    let mut source = vec![0.; 64 * 64 * 4];
    let mut target = vec![0.; source.len()];
    for y in 0..64 {
        for x in 0..64 {
            let value = (0.5 + 0.2 * (x as f64 * 0.31 + y as f64 * 0.17).sin()
                + 0.15 * (x as f64 * 0.11 - y as f64 * 0.41).cos()) as f32;
            let i = (y * 64 + x) * 4;
            source[i..i + 4].copy_from_slice(&[value, value, value, 1.]);
            let j = (y * 64 + 63 - x) * 4;
            target[j..j + 4].copy_from_slice(&[value, value, value, 1.]);
        }
    }
    let a = LinearRgbaView::new(64, 64, &source, 4096, || false).unwrap();
    let b = LinearRgbaView::new(64, 64, &target, 4096, || false).unwrap();
    for describe in [describe_gradient, describe_gradient_interpolated] {
        for angle in [0., 0.37, 1.1] {
            let original = describe(&a, [32.25, 31.75], 1., angle, 256, never).unwrap().unwrap();
            let mirrored = describe(&b, [30.75, 31.75], 1., std::f64::consts::PI - angle, 256, never).unwrap().unwrap();
            let transformed = reflect_gradient_descriptor(&original, || false).unwrap();
            let distance: f64 = transformed.0.iter().zip(mirrored.0.iter()).map(|(x, y)| (x - y).powi(2)).sum();
            assert!(distance < 1e-24, "angle={angle}: {distance}");
            assert_eq!(reflect_gradient_descriptor(&transformed, || false).unwrap(), original);
            let calls = Cell::new(0);
            reflect_gradient_descriptor(&original, || { calls.set(calls.get() + 1); false }).unwrap();
            for stop in [1, calls.get() / 2, calls.get()] {
                let seen = Cell::new(0);
                assert_eq!(reflect_gradient_descriptor(&original, || { seen.set(seen.get() + 1); seen.get() == stop }), Err(LocalError::Cancelled));
            }
            let mut invalid = original;
            invalid.0[0] = f64::NAN;
            assert_eq!(reflect_gradient_descriptor(&invalid, || false), Err(LocalError::Invalid));
            assert_eq!(reflect_gradient_descriptor(&invalid, || true), Err(LocalError::Cancelled));
        }
    }
}
