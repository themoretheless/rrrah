use rrrah_dedup::{
    crop::{CropBudget, CropError, Region, find_exact_crops},
    linear::LinearRgbaView,
};

const BUDGET: CropBudget = CropBudget {
    max_pixel_comparisons: 100_000,
    max_matches: 100,
};

#[test]
fn bordered_hdr_image_matches_only_the_full_crop() {
    let image: Vec<f32> = (0_u16..30).flat_map(|i| [f32::from(i), 0.0, 0.0, 1.0]).collect();
    let needle: Vec<f32> = [8_usize, 9, 14, 15]
        .into_iter()
        .flat_map(|i| image[i * 4..i * 4 + 4].iter().copied())
        .collect();
    let a = LinearRgbaView::new(6, 5, &image, 30, || false).unwrap();
    let b = LinearRgbaView::new(2, 2, &needle, 4, || false).unwrap();
    let result = find_exact_crops(&a, &b, BUDGET, || false).unwrap();
    assert_eq!(
        result.placements,
        [Region {
            x: 2,
            y: 1,
            width: 2,
            height: 2
        }]
    );
    assert!(result.informative);
    let mut changed = needle;
    changed[12] += 0.001;
    let c = LinearRgbaView::new(2, 2, &changed, 4, || false).unwrap();
    assert!(
        find_exact_crops(&a, &c, BUDGET, || false)
            .unwrap()
            .placements
            .is_empty()
    );
}

#[test]
fn uniform_transparent_matches_expose_ambiguity_and_result_limits() {
    let image = [0.0; 24];
    let a = LinearRgbaView::new(3, 2, &image, 6, || false).unwrap();
    let b = LinearRgbaView::new(1, 1, &[f32::NAN, 99.0, 33.0, 0.0], 1, || false).unwrap();
    let result = find_exact_crops(&a, &b, BUDGET, || false).unwrap();
    assert_eq!(result.placements.len(), 6);
    assert!(!result.informative);
    let exact_budget = CropBudget {
        max_matches: 6,
        ..BUDGET
    };
    let calls = std::cell::Cell::new(0);
    let exact = find_exact_crops(&a, &b, exact_budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(exact.placements, result.placements);
    for checkpoint in 1..=calls.get() {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            find_exact_crops(&a, &b, exact_budget, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(CropError::Cancelled)
        );
    }
    assert_eq!(find_exact_crops(&a, &b, exact_budget, || false).unwrap(), exact);
    assert_eq!(
        find_exact_crops(
            &a,
            &b,
            CropBudget {
                max_matches: 1,
                ..BUDGET
            },
            || false
        ),
        Err(CropError::Budget)
    );
}

#[test]
fn work_budget_and_mid_search_cancel_are_inconclusive() {
    use std::cell::Cell;
    let samples = [1.0; 16];
    let a = LinearRgbaView::new(2, 2, &samples, 4, || false).unwrap();
    assert_eq!(
        find_exact_crops(
            &a,
            &a,
            CropBudget {
                max_pixel_comparisons: 3,
                ..BUDGET
            },
            || false
        ),
        Err(CropError::Budget)
    );
    let calls = Cell::new(0);
    assert_eq!(
        find_exact_crops(&a, &a, BUDGET, || {
            calls.set(calls.get() + 1);
            calls.get() > 5
        }),
        Err(CropError::Cancelled)
    );
}
