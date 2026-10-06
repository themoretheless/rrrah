use rrrah_dedup::pixels::{PixelError, Rgba8View};

#[test]
fn validates_layout_budget_and_cancellation() {
    assert_eq!(Rgba8View::new(0, 1, &[], 10).unwrap_err(), PixelError::Layout);
    assert_eq!(Rgba8View::new(1, 1, &[0; 3], 10).unwrap_err(), PixelError::Layout);
    assert_eq!(Rgba8View::new(1, 1, &[0; 4], 0).unwrap_err(), PixelError::Budget);
    let view = Rgba8View::new(1, 1, &[0; 4], 1).unwrap();
    assert_eq!(view.pixel_digest(|| true), Err(PixelError::Cancelled));
    assert_eq!(view.fingerprint(|| true), Err(PixelError::Cancelled));
    assert_eq!(view.same_pixels(&view, || true), Err(PixelError::Cancelled));
}

#[test]
fn invisible_rgb_is_canonical_but_visible_pixels_are_exact() {
    let a = Rgba8View::new(1, 1, &[10, 20, 30, 0], 1).unwrap();
    let b = Rgba8View::new(1, 1, &[90, 80, 70, 0], 1).unwrap();
    let c = Rgba8View::new(1, 1, &[10, 20, 30, 1], 1).unwrap();
    assert!(a.same_pixels(&b, || false).unwrap());
    assert_eq!(a.pixel_digest(|| false), b.pixel_digest(|| false));
    assert!(!a.same_pixels(&c, || false).unwrap());
    assert_ne!(a.pixel_digest(|| false), c.pixel_digest(|| false));
}

#[test]
fn gradient_has_independently_known_difference_bits() {
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    for y in 0_u8..9 {
        for x in 0_u8..9 {
            horizontal.extend_from_slice(&[x * 28, x * 28, x * 28, 255]);
            vertical.extend_from_slice(&[y * 28, y * 28, y * 28, 255]);
        }
    }
    let a = Rgba8View::new(9, 9, &horizontal, 81)
        .unwrap()
        .fingerprint(|| false)
        .unwrap();
    let b = Rgba8View::new(9, 9, &vertical, 81)
        .unwrap()
        .fingerprint(|| false)
        .unwrap();
    assert_eq!(a.variants[0], [u64::MAX, 0, u64::MAX, 0]);
    assert_eq!(b.variants[0], [0, u64::MAX, 0, u64::MAX]);
    assert_eq!(a.compare(&b, true).distance, 0);
    assert_eq!(a.compare(&b, false).distance, 256);
    assert!(a.compare(&b, true).informative);
}

#[test]
fn uniform_hash_collision_is_not_informative() {
    let a = Rgba8View::new(1, 1, &[0, 0, 0, 255], 1)
        .unwrap()
        .fingerprint(|| false)
        .unwrap();
    let b = Rgba8View::new(1, 1, &[255, 255, 255, 255], 1)
        .unwrap()
        .fingerprint(|| false)
        .unwrap();
    assert!(!a.compare(&b, true).informative);
    assert!(a.mean_linear_rgb[0] < 0.01);
    assert!(b.mean_linear_rgb[0] > 0.99);
}
