use rrrah_dedup::{linear::LinearRgbaView, pixels::PixelError};

#[test]
fn hdr_and_sub_byte_differences_are_retained() {
    let a = LinearRgbaView::new(1, 1, &[2.0, -0.1, 0.1234, 1.0], 1, || false).unwrap();
    let b = LinearRgbaView::new(1, 1, &[2.1, -0.1, 0.1234, 1.0], 1, || false).unwrap();
    let c = LinearRgbaView::new(1, 1, &[2.0, -0.1, 0.1235, 1.0], 1, || false).unwrap();
    assert_eq!(a.rgba(0, 0), Some([2.0, -0.1, 0.1234, 1.0]));
    assert_eq!(a.rgba(1, 0), None);
    assert_eq!(a.rgba(0, 1), None);
    for other in [b, c] {
        assert!(!a.same_pixels(&other, || false).unwrap());
        assert_ne!(a.pixel_digest(|| false), other.pixel_digest(|| false));
    }
}

#[test]
fn invisible_rgb_and_signed_zero_are_canonical() {
    let a = LinearRgbaView::new(
        2,
        1,
        &[f32::NAN, 99.0, f32::INFINITY, 0.0, -0.0, 0.0, 1.0, 1.0],
        2,
        || false,
    )
    .unwrap();
    let b = LinearRgbaView::new(2, 1, &[0.0, 0.0, 0.0, -0.0, 0.0, -0.0, 1.0, 1.0], 2, || false).unwrap();
    assert_eq!(a.rgba(0, 0), Some([0.0; 4]));
    assert_eq!(a.rgba(1, 0).unwrap()[0].to_bits(), 0);
    assert!(a.same_pixels(&b, || false).unwrap());
    assert_eq!(a.pixel_digest(|| false), b.pixel_digest(|| false));
}

#[test]
fn invalid_samples_budget_and_cancel_are_explicit() {
    for rgba in [
        [f32::NAN, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, f32::NAN],
        [0.0, 0.0, 0.0, 1.1],
    ] {
        assert_eq!(
            LinearRgbaView::new(1, 1, &rgba, 1, || false).unwrap_err(),
            PixelError::Layout
        );
    }
    assert_eq!(
        LinearRgbaView::new(1, 1, &[0.0; 4], 0, || false).unwrap_err(),
        PixelError::Budget
    );
    assert_eq!(
        LinearRgbaView::new(1, 1, &[0.0; 4], 1, || true).unwrap_err(),
        PixelError::Cancelled
    );
}

#[test]
fn hdr_fingerprints_keep_highlight_structure_without_quantization() {
    let samples: Vec<f32> = (0_u8..9)
        .flat_map(|_| {
            (0_u8..9).flat_map(|x| {
                let value = 2.0 + f32::from(x);
                [value, value, value, 1.0]
            })
        })
        .collect();
    let view = LinearRgbaView::new(9, 9, &samples, 81, || false).unwrap();
    let fp = view.fingerprint(|| false).unwrap();
    assert_eq!(fp.variants[0], [u64::MAX, 0, u64::MAX, 0]);
    assert!(fp.mean_linear_rgb[0] > 5.9);
    assert!(fp.luminance_stddev > 2.0);
    assert_eq!(view.fingerprint(|| true), Err(PixelError::Cancelled));
}
