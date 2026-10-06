use rrrah_dedup::{
    geometry::{GeometryPolicy, verify_similarity},
    linear::LinearRgbaView,
    local::{LocalError, LocalPolicy, MatchPolicy, extract_spatial_oriented, match_features},
};
fn pixels(mut seed: u64) -> Vec<f32> {
    (0..96 * 96)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let value = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [value, value, value, 1.0]
        })
        .collect()
}
#[test]
fn spatial_quotas_keep_self_geometry_and_reject_unrelated_texture() {
    let p = LocalPolicy {
        max_pixels: 10000,
        max_candidates: 10000,
        max_features: 100,
        minimum_corner_score: 0.0001,
    };
    let source = pixels(1_234_567);
    let other = pixels(9_876_543);
    let image = LinearRgbaView::new(96, 96, &source, 10000, || false).unwrap();
    let unrelated = LinearRgbaView::new(96, 96, &other, 10000, || false).unwrap();
    let left = extract_spatial_oriented(&image, p, 4, 3, 3, || false).unwrap();
    assert!(left.len() >= 10 && left.len() <= 36);
    let mut cells = [0; 12];
    for f in &left {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cell = (f.position[1] as usize * 3 / 96) * 4 + f.position[0] as usize * 4 / 96;
        cells[cell] += 1;
    }
    assert!(cells.into_iter().all(|count| count <= 3));
    let right = extract_spatial_oriented(&unrelated, p, 4, 3, 3, || false).unwrap();
    let matching = MatchPolicy {
        max_comparisons: 10000,
        max_distance: 64,
    };
    let self_matches = match_features(&left, &left, matching, || false).unwrap();
    let geometry = GeometryPolicy {
        tolerance: 0.01,
        min_inliers: 10,
        max_points: 100,
        max_hypotheses: 5000,
    };
    assert!(
        verify_similarity(&self_matches, geometry, || false)
            .unwrap()
            .is_some()
    );
    let negatives = match_features(&left, &right, matching, || false).unwrap();
    assert!(
        verify_similarity(&negatives, geometry, || false)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        extract_spatial_oriented(&image, p, 0, 3, 3, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        extract_spatial_oriented(&image, p, 100, 100, 3, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        extract_spatial_oriented(&image, p, 4, 3, 3, || true),
        Err(LocalError::Cancelled)
    ));
}
