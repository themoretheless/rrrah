use rrrah_dedup::{
    crop::{CropBudget, CropError, Region, find_exact_crops},
    linear::LinearRgbaView,
};

const BUDGET: CropBudget = CropBudget {
    max_pixel_comparisons: 100_000,
    max_matches: 100,
};

#[test]
fn bottom_right_crop_checks_every_color_and_alpha_without_padding() {
    let image: Vec<f32> = (1_u8..=12)
        .flat_map(|i| [7.0, f32::from(i), f32::from(i) / 2.0, 1.0])
        .collect();
    let needle = [
        7.0, 7.0, 3.5, 1.0, 7.0, 8.0, 4.0, 1.0, 7.0, 11.0, 5.5, 1.0, 7.0, 12.0, 6.0, 1.0,
    ];
    let source = LinearRgbaView::new(4, 3, &image, 12, || false).unwrap();
    let crop = LinearRgbaView::new(2, 2, &needle, 4, || false).unwrap();
    assert_eq!(
        find_exact_crops(&source, &crop, BUDGET, || false)
            .unwrap()
            .placements,
        [Region {
            x: 2,
            y: 1,
            width: 2,
            height: 2
        }]
    );
    // Red is identical everywhere. Every green/blue/alpha sample still matters,
    // including the bottom-right source pixel at both inclusive image limits.
    for pixel in 0..4 {
        for channel in [1, 2, 3] {
            let mut changed = needle;
            changed[pixel * 4 + channel] += if channel == 3 { -0.5 } else { 0.25 };
            let changed = LinearRgbaView::new(2, 2, &changed, 4, || false).unwrap();
            assert!(
                find_exact_crops(&source, &changed, BUDGET, || false)
                    .unwrap()
                    .placements
                    .is_empty(),
                "pixel={pixel}, channel={channel}"
            );
        }
    }
    assert_eq!(
        find_exact_crops(&source, &source, BUDGET, || false)
            .unwrap()
            .placements,
        [Region {
            x: 0,
            y: 0,
            width: 4,
            height: 3
        }]
    );
    let mut padded = vec![0.0; 5 * 4 * 4];
    for y in 0..3 {
        padded[y * 20..y * 20 + 16].copy_from_slice(&image[y * 16..y * 16 + 16]);
    }
    let padded = LinearRgbaView::new(5, 4, &padded, 20, || false).unwrap();
    let absent = find_exact_crops(&source, &padded, BUDGET, || false).unwrap();
    assert!(absent.placements.is_empty());
    assert_eq!(absent.compared_pixels, 0);
}

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
