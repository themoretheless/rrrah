use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy, WarpPolicy,
        verify_filtered_photometric_with_color,
    },
};
const PHOTO: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 500,
    },
    minimum_samples: 16,
    minimum_variance: 1e-5,
    minimum_gain: 0.2,
    maximum_gain: 5.,
    maximum_offset: 1.,
};
const FILTER: ColorFilterPolicy = ColorFilterPolicy {
    filter: FilterPolicy {
        radius: 3,
        max_sample_pairs: 56_644,
    },
    color_space: FilterColorSpace::EncodedSrgb,
};
fn pixels(name: &str) -> Vec<f32> {
    let bytes = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/encoded-srgb/{name}.rgba32le")),
    )
    .unwrap();
    assert_eq!(bytes.len(), 17 * 17 * 4 * 4);
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}
#[test]
fn encoded_affine_changes_match_independent_decimal_fixtures_and_preserve_linear_identity() {
    let a = pixels("source");
    let b = pixels("affine");
    let av = LinearRgbaView::new(17, 17, &a, 289, || false).unwrap();
    let bv = LinearRgbaView::new(17, 17, &b, 289, || false).unwrap();
    assert!(!av.same_pixels(&bv, || false).unwrap());
    let e = verify_filtered_photometric_with_color(
        &av,
        &bv,
        Transform {
            a: 1.,
            b: 0.,
            translation: [0.; 2],
        },
        PHOTO,
        FILTER,
        || false,
    )
    .unwrap();
    assert_eq!(e.color_space, FilterColorSpace::EncodedSrgb);
    for p in [&e.evidence.forward, &e.evidence.reverse] {
        assert_eq!(
            (
                p.fitted_samples,
                p.pixels.compared_pixels,
                p.pixels.matched_pixels
            ),
            (121, 121, 121)
        );
    }
    for (a, b) in e.evidence.forward.gain.into_iter().zip([0.7; 3]) {
        assert!((a - b).abs() < 1e-6);
    }
    for (a, b) in e.evidence.forward.offset.into_iter().zip([0.02; 3]) {
        assert!((a - b).abs() < 1e-6);
    }
}
#[test]
fn seven_by_seven_encoded_windows_retain_broad_color_and_alpha_edits() {
    let a = pixels("source");
    let av = LinearRgbaView::new(17, 17, &a, 289, || false).unwrap();
    for name in ["broad-edit", "alpha-edit"] {
        let b = pixels(name);
        let bv = LinearRgbaView::new(17, 17, &b, 289, || false).unwrap();
        let e = verify_filtered_photometric_with_color(
            &av,
            &bv,
            Transform {
                a: 1.,
                b: 0.,
                translation: [0.; 2],
            },
            PHOTO,
            FILTER,
            || false,
        )
        .unwrap();
        for p in [&e.evidence.forward, &e.evidence.reverse] {
            assert!(
                p.pixels.matched_pixels * 10 < p.pixels.compared_pixels * 9,
                "{name}"
            );
        }
    }
}
