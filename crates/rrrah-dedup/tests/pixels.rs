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

#[test]
fn all_opaque_gray_levels_collide_only_as_uninformative_candidates() {
    let pixels: Vec<_> = (0..=255_u8).map(|shade| [shade, shade, shade, 255]).collect();
    let views: Vec<_> = pixels
        .iter()
        .map(|rgba| Rgba8View::new(1, 1, rgba, 1).unwrap())
        .collect();
    let fingerprints: Vec<_> = views
        .iter()
        .map(|view| view.fingerprint(|| false).unwrap())
        .collect();
    let digests: std::collections::BTreeSet<_> = views
        .iter()
        .map(|view| view.pixel_digest(|| false).unwrap())
        .collect();
    assert_eq!(digests.len(), 256);
    for (left, first) in views.iter().enumerate() {
        for (right, second) in views.iter().enumerate() {
            let comparison = fingerprints[left].compare(&fingerprints[right], true);
            assert_eq!(comparison.distance, 0);
            assert!(!comparison.informative);
            assert_eq!(first.same_pixels(second, || false).unwrap(), left == right);
        }
    }
}

#[test]
fn identical_and_transparent_pixel_equality_observe_late_cancellation() {
    use std::cell::Cell;
    for count in [1_u32, 4096, 4097, 8193] {
        let mut a = vec![37_u8; count as usize * 4];
        for pixel in a.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        for transparent in [false, true] {
            let mut left = a.clone();
            let mut right = a.clone();
            if transparent {
                let offset = (count as usize / 2) * 4;
                left[offset..offset + 4].copy_from_slice(&[1, 2, 3, 0]);
                right[offset..offset + 4].copy_from_slice(&[9, 8, 7, 0]);
            }
            let first = Rgba8View::new(count, 1, &left, u64::from(count)).unwrap();
            let second = Rgba8View::new(count, 1, &right, u64::from(count)).unwrap();
            assert_eq!(first.same_pixels(&second, || false), Ok(true));
            let calls = Cell::new(0);
            let result = first.same_pixels(&second, || {
                calls.set(calls.get() + 1);
                calls.get() == 2
            });
            assert_eq!(
                result,
                Err(PixelError::Cancelled),
                "count={count}, transparent={transparent}"
            );
            assert_eq!(calls.get(), 2);
        }
    }
}

#[test]
fn identical_pixels_honor_each_block_and_final_cancellation_checkpoint() {
    use std::cell::Cell;
    let bytes = vec![255; 8193 * 4];
    let view = Rgba8View::new(8193, 1, &bytes, 8193).unwrap();
    let calls = Cell::new(0);
    assert_eq!(
        view.same_pixels(&view, || {
            calls.set(calls.get() + 1);
            false
        }),
        Ok(true)
    );
    let total = calls.get();
    assert!(
        total >= 3,
        "equality requires intermediate and final cancellation checks"
    );
    for stop in 1..=total {
        calls.set(0);
        assert_eq!(
            view.same_pixels(&view, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(PixelError::Cancelled),
            "checkpoint={stop}"
        );
        assert_eq!(calls.get(), stop);
    }
}

#[test]
fn transparent_texture_is_canonical_and_alpha_gradient_survives_white_projection() {
    let mut hidden_a = Vec::new();
    let mut hidden_b = Vec::new();
    let mut alpha_rising = Vec::new();
    let mut alpha_falling = Vec::new();
    for y in 0_u8..9 {
        for x in 0_u8..9 {
            hidden_a.extend_from_slice(&[x * 28, y * 28, 17, 0]);
            hidden_b.extend_from_slice(&[255 - x * 28, 255 - y * 28, 231, 0]);
            alpha_rising.extend_from_slice(&[0, 0, 0, x * 28]);
            alpha_falling.extend_from_slice(&[0, 0, 0, 224 - x * 28]);
        }
    }
    let view = |bytes| Rgba8View::new(9, 9, bytes, 81).unwrap();
    let a = view(&hidden_a);
    let b = view(&hidden_b);
    assert!(a.same_pixels(&b, || false).unwrap());
    assert_eq!(a.pixel_digest(|| false), b.pixel_digest(|| false));
    let fa = a.fingerprint(|| false).unwrap();
    let fb = b.fingerprint(|| false).unwrap();
    assert_eq!(fa, fb);
    assert!(!fa.compare(&fb, true).informative);
    let rising = view(&alpha_rising);
    let falling = view(&alpha_falling);
    assert!(!rising.same_pixels(&falling, || false).unwrap());
    let fr = rising.fingerprint(|| false).unwrap();
    let ff = falling.fingerprint(|| false).unwrap();
    // Black RGB projects to zero on black for every alpha. On white the
    // signal is 1-alpha: only the reversed alpha ramp rises horizontally.
    assert_eq!(fr.variants[0], [0, 0, 0, 0]);
    assert_eq!(ff.variants[0], [0, 0, u64::MAX, 0]);
    assert_eq!(fr.compare(&ff, false).distance, 64);
}
