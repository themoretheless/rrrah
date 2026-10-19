#[cfg(test)]
mod tests {
    use hayro::hayro_interpret::encode::{OrderedPixel, PixelPartitionError, PixelSourceOwner};
    use kurbo::Point;
    fn owner(index: usize, color: [f64; 3]) -> PixelSourceOwner {
        PixelSourceOwner {
            patch_index: index,
            u: [0.; 3],
            v: [0.; 3],
            component_count: 3,
            components: [
                [color[0], 0., 0.],
                [color[1], 0., 0.],
                [color[2], 0., 0.],
                [0.; 3],
            ],
        }
    }
    fn rectangle(pixel: &mut OrderedPixel<'_>, low: f64, high: f64, source: PixelSourceOwner) {
        let p = [
            Point::new(low, 0.),
            Point::new(high, 0.),
            Point::new(high, 1.),
            Point::new(low, 1.),
        ];
        for points in [[p[0], p[1], p[2]], [p[0], p[2], p[3]]] {
            pixel
                .paint_triangle(points, source, &mut 1_000_000, &|| false)
                .unwrap();
        }
    }
    fn integrated(pixel: &OrderedPixel<'_>) -> [f64; 4] {
        let mut sum = [0.; 4];
        pixel
            .visit(&mut 1_000_000, &|| false, &mut |area, center, polygon, source| {
                assert!((3..=16).contains(&polygon.len()));
                assert!(
                    polygon
                        .iter()
                        .all(|p| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y))
                );
                let values = source.evaluate(center).ok_or(PixelPartitionError::Invalid)?;
                for i in 0..3 {
                    sum[i] += values[i] * area;
                }
                sum[3] += area;
                Ok(())
            })
            .unwrap();
        sum
    }
    fn close(actual: [f64; 4], expected: [f64; 4]) {
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{actual:?} != {expected:?}");
        }
    }
    #[test]
    fn ordered_pixel_unions_coverage_and_preserves_later_patch_components() {
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut first = OrderedPixel::new(&admit, 256);
        rectangle(&mut first, 0., 1.5, owner(0, [1., 0., 0.]));
        rectangle(&mut first, 0.5, 2., owner(1, [0., 0., 1.]));
        close(integrated(&first), [0.5, 0., 0.5, 1.]);
        let mut second = OrderedPixel::new(&admit, 256);
        rectangle(&mut second, -1., 0.5, owner(0, [1., 0., 0.]));
        rectangle(&mut second, -0.5, 1., owner(1, [0., 0., 1.]));
        close(integrated(&second), [0., 0., 1., 1.]);
        // Earlier source submitted later must not replace the later patch.
        rectangle(&mut second, 0., 1., owner(0, [1., 0., 0.]));
        close(integrated(&second), [0., 0., 1., 1.]);
        drop((first, second));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn ordered_pixel_folded_priority_uses_v_then_u_and_not_multiplicity() {
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut pixel = OrderedPixel::new(&admit, 256);
        let points = [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)];
        let mut red = owner(0, [1., 0., 0.]);
        red.u = [0.25, 0., 0.];
        let mut blue = owner(0, [0., 0., 1.]);
        blue.u = [0.75, 0., 0.];
        pixel
            .paint_triangle(points, blue, &mut 1_000_000, &|| false)
            .unwrap();
        pixel
            .paint_triangle(points, red, &mut 1_000_000, &|| false)
            .unwrap();
        close(integrated(&pixel), [0., 0., 0.5, 0.5]);
        drop(pixel);
        for tie_v in [false, true] {
            let mut pixel = OrderedPixel::new(&admit, 256);
            let mut red = owner(0, [1., 0., 0.]);
            let mut blue = owner(0, [0., 0., 1.]);
            if tie_v {
                red.u = [0., 1., 0.];
                blue.u = [1., -1., 0.];
                red.v = [0., 0., 1.];
                blue.v = red.v;
            } else {
                red.v = [0., 1., 0.];
                blue.v = [1., -1., 0.];
            }
            rectangle(&mut pixel, 0., 1., red);
            rectangle(&mut pixel, 0., 1., blue);
            close(integrated(&pixel), [0.5, 0., 0.5, 1.]);
        }
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn ordered_pixel_affine_components_integrate_over_winning_fragments() {
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut pixel = OrderedPixel::new(&admit, 256);
        let mut ramp = owner(0, [0.; 3]);
        ramp.components[0] = [0., 1., 0.];
        rectangle(&mut pixel, 0., 1., ramp);
        rectangle(&mut pixel, 0.5, 1., owner(1, [0., 0., 1.]));
        close(integrated(&pixel), [0.125, 0., 0.5, 1.]);
        drop(pixel);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn ordered_pixel_every_admission_failure_cancel_and_work_limit_are_transactional() {
        use std::cell::Cell;
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let calls = Cell::new(0);
        let refuse = Cell::new(usize::MAX);
        let admit = |bytes: usize| {
            calls.set(calls.get() + 1);
            if calls.get() == refuse.get() {
                None
            } else {
                root.try_reserve(bytes as u64)
                    .ok()
                    .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
            }
        };
        let points = [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)];
        let attempts = {
            let mut pixel = OrderedPixel::new(&admit, 256);
            rectangle(&mut pixel, 0., 1., owner(0, [1., 0., 0.]));
            calls.set(0);
            pixel
                .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| false)
                .unwrap();
            calls.get()
        };
        assert!(attempts > 0);
        assert_eq!(root.used(), 0);
        for failure in 1..=attempts {
            refuse.set(usize::MAX);
            let mut pixel = OrderedPixel::new(&admit, 256);
            rectangle(&mut pixel, 0., 1., owner(0, [1., 0., 0.]));
            let before = root.used();
            calls.set(0);
            refuse.set(failure);
            assert_eq!(
                pixel
                    .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| false)
                    .unwrap_err(),
                PixelPartitionError::Admission
            );
            close(integrated(&pixel), [1., 0., 0., 1.]);
            assert_eq!(root.used(), before);
            refuse.set(usize::MAX);
            assert_eq!(
                pixel
                    .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1, &|| false)
                    .unwrap_err(),
                PixelPartitionError::Limit
            );
            assert_eq!(
                pixel
                    .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| true)
                    .unwrap_err(),
                PixelPartitionError::Cancelled
            );
            close(integrated(&pixel), [1., 0., 0., 1.]);
            assert_eq!(root.used(), before);
            pixel
                .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| false)
                .unwrap();
            close(integrated(&pixel), [0.5, 0., 0.5, 1.]);
        }
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn ordered_pixel_every_mid_partition_cancel_preserves_published_regions() {
        use std::cell::Cell;
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let points = [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)];
        let polls = Cell::new(0);
        {
            let mut pixel = OrderedPixel::new(&admit, 256);
            rectangle(&mut pixel, 0., 1., owner(0, [1., 0., 0.]));
            pixel
                .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap();
        }
        let checkpoints = polls.get();
        assert!(checkpoints > 20);
        for stop in 1..=checkpoints {
            let mut pixel = OrderedPixel::new(&admit, 256);
            rectangle(&mut pixel, 0., 1., owner(0, [1., 0., 0.]));
            let before = root.used();
            polls.set(0);
            assert_eq!(
                pixel
                    .paint_triangle(points, owner(1, [0., 0., 1.]), &mut 1_000_000, &|| {
                        polls.set(polls.get() + 1);
                        polls.get() == stop
                    })
                    .unwrap_err(),
                PixelPartitionError::Cancelled
            );
            close(integrated(&pixel), [1., 0., 0., 1.]);
            assert_eq!(root.used(), before);
        }
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn ordered_pixel_fragment_limits_and_invalid_input_preserve_previous_source() {
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut pixel = OrderedPixel::new(&admit, 1);
        pixel
            .paint_triangle(
                [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)],
                owner(0, [1., 0., 0.]),
                &mut 1_000_000,
                &|| false,
            )
            .unwrap();
        let before = root.used();
        assert_eq!(
            pixel
                .paint_triangle(
                    [Point::new(0., 0.25), Point::new(0.25, 0.25), Point::new(0., 0.5)],
                    owner(1, [0., 0., 1.]),
                    &mut 1_000_000,
                    &|| false
                )
                .unwrap_err(),
            PixelPartitionError::Limit
        );
        assert_eq!(
            pixel
                .paint_triangle(
                    [Point::new(f64::NAN, 0.), Point::new(1., 0.), Point::new(0., 1.)],
                    owner(1, [0., 0., 1.]),
                    &mut 1_000_000,
                    &|| false
                )
                .unwrap_err(),
            PixelPartitionError::Invalid
        );
        close(integrated(&pixel), [0.5, 0., 0., 0.5]);
        assert_eq!(root.used(), before);
        drop(pixel);
        assert_eq!(root.used(), 0);
    }
}
