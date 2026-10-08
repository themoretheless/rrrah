#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{geometry::ProjectiveTransform, local::LocalError, region_grid::projective_grid_domains};
const ID: ProjectiveTransform = ProjectiveTransform {
    matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
};
#[test]
fn irregular_identity_grid_partitions_and_owned_credit() {
    let bytes = 6 * std::mem::size_of::<[rrrah_dedup::warp::PixelRectangle; 2]>() as u64;
    let budget = MemoryBudget::new(bytes);
    let result = projective_grid_domains((11, 7), (11, 7), ID, (3, 2), 6, &budget, || false).unwrap();
    assert_eq!(result.len(), 6);
    let mut cover = vec![0; 77];
    for pair in result.iter() {
        assert_eq!(pair[0], pair[1]);
        let r = pair[0];
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                cover[(y * 11 + x) as usize] += 1;
            }
        }
    }
    assert!(cover.iter().all(|n| *n == 1));
    assert_eq!(budget.used(), bytes);
    drop(result);
    assert_eq!(budget.used(), 0);
    let short = MemoryBudget::new(bytes - 1);
    assert!(matches!(
        projective_grid_domains((11, 7), (11, 7), ID, (3, 2), 6, &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
}
#[test]
fn horizons_limits_and_final_cancellation_refuse_atomically() {
    let budget = MemoryBudget::new(4096);
    let horizon = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [1., 0., -5.]],
    };
    assert!(matches!(
        projective_grid_domains((11, 7), (11, 7), horizon, (3, 2), 6, &budget, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        projective_grid_domains((11, 7), (11, 7), ID, (3, 2), 5, &budget, || false),
        Err(LocalError::Budget)
    ));
    let calls = std::cell::Cell::new(0);
    drop(
        projective_grid_domains((11, 7), (11, 7), ID, (3, 2), 6, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            projective_grid_domains((11, 7), (11, 7), ID, (3, 2), 6, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn independent_translation_quarter_turn_and_small_image_domains() {
    let budget = MemoryBudget::new(4096);
    let shifted = ProjectiveTransform {
        matrix: [[1., 0., 2.], [0., 1., 0.], [0., 0., 1.]],
    };
    let clipped = projective_grid_domains((5, 3), (5, 3), shifted, (1, 1), 1, &budget, || false).unwrap();
    assert_eq!(clipped.len(), 1);
    let [source, target] = clipped[0];
    assert_eq!([source.x, source.y, source.width, source.height], [0, 0, 5, 3]);
    assert_eq!([target.x, target.y, target.width, target.height], [2, 0, 3, 3]);
    drop(clipped);
    assert_eq!(budget.used(), 0);
    let rotated = ProjectiveTransform {
        matrix: [[0., -1., 2.], [1., 0., 0.], [0., 0., 1.]],
    };
    let cells = projective_grid_domains((5, 3), (3, 5), rotated, (5, 3), 15, &budget, || false).unwrap();
    assert_eq!(cells.len(), 15);
    for (i, pair) in cells.iter().enumerate() {
        let x = (i % 5) as u32;
        let y = (i / 5) as u32;
        assert_eq!(
            [pair[0].x, pair[0].y, pair[0].width, pair[0].height],
            [x, y, 1, 1]
        );
        assert_eq!(
            [pair[1].x, pair[1].y, pair[1].width, pair[1].height],
            [2 - y, x, 1, 1]
        );
    }
    drop(cells);
    assert_eq!(budget.used(), 0);
    let tiny = projective_grid_domains((2, 2), (2, 2), ID, (4, 4), 16, &budget, || false).unwrap();
    assert_eq!(tiny.len(), 4);
    assert!(
        tiny.iter()
            .all(|pair| pair[0] == pair[1] && pair[0].width == 1 && pair[0].height == 1)
    );
    drop(tiny);
    assert_eq!(budget.used(), 0);
    let missing = MemoryBudget::new(0);
    assert!(matches!(
        projective_grid_domains((0, 3), (5, 3), ID, (1, 1), 1, &missing, || false),
        Err(LocalError::Invalid)
    ));
    assert_eq!(missing.peak(), 0);
}

#[test]
fn bidirectional_translation_covers_clipped_overlap_and_cancels_atomically() {
    use rrrah_dedup::region_grid::projective_bidirectional_grid_domains;
    let transform = ProjectiveTransform {
        matrix: [[1., 0., 2.], [0., 1., 0.], [0., 0., 1.]],
    };
    let bytes = std::mem::size_of::<[rrrah_dedup::warp::PixelRectangle; 2]>() as u64;
    let budget = MemoryBudget::new(3 * bytes);
    let calls = std::cell::Cell::new(0usize);
    let result = projective_bidirectional_grid_domains((5, 3), (5, 3), transform, (1, 1), 2, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(result.len(), 2);
    let rectangles: Vec<_> = result
        .iter()
        .map(|pair| pair.map(|r| [r.x, r.y, r.width, r.height]))
        .collect();
    assert_eq!(
        rectangles,
        vec![[[0, 0, 5, 3], [2, 0, 3, 3]], [[0, 0, 3, 3], [0, 0, 5, 3]]]
    );
    // Right-grid candidate contains the full clipped left overlap, unlike the
    // first candidate whose left rectangle includes pixels outside the target.
    assert_eq!(budget.used(), 2 * bytes);
    assert_eq!(budget.peak(), 3 * bytes);
    let checkpoints = calls.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in 0..checkpoints {
        calls.set(0);
        assert!(matches!(
            projective_bidirectional_grid_domains((5, 3), (5, 3), transform, (1, 1), 2, &budget, || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            }),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let short = MemoryBudget::new(3 * bytes - 1);
    assert!(matches!(
        projective_bidirectional_grid_domains((5, 3), (5, 3), transform, (1, 1), 2, &short, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(short.used(), 0);
    let zero = MemoryBudget::new(0);
    assert!(matches!(
        projective_bidirectional_grid_domains((5, 3), (5, 3), transform, (1, 1), 1, &zero, || false),
        Err(LocalError::Budget)
    ));
    assert_eq!(zero.peak(), 0);
}
