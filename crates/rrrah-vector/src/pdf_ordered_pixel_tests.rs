#[cfg(test)]
mod tests {
    use hayro::hayro_interpret::encode::{OrderedPixel, PixelPartitionError, PixelSourceOwner};
    use kurbo::Point;
    #[test]
    fn coons_affine_identity_checks_original_boundary_controls() {
        use hayro::hayro_interpret::{encode::PatchParameterIdentity, shading::CoonsPatch};
        let uv = [
            [0., 0.],
            [0., 1.],
            [0., 2.],
            [0., 3.],
            [1., 3.],
            [2., 3.],
            [3., 3.],
            [3., 2.],
            [3., 1.],
            [3., 0.],
            [2., 0.],
            [1., 0.],
        ];
        let mut patch = CoonsPatch {
            control_points: uv.map(|[u, v]| Point::new(2. * u + v, u + 3. * v)),
            colors: Default::default(),
        };
        let identity = PatchParameterIdentity::coons(&patch, 0, kurbo::Affine::IDENTITY).unwrap();
        assert!(identity.has_u() && identity.has_v());
        let mut source = owner(0, [0.; 3]);
        identity.apply(&mut source, Point::ZERO).unwrap();
        for i in 0..=16 {
            for j in 0..=16 {
                let u = i as f64 / 16.;
                let v = j as f64 / 16.;
                let point = patch.map_coordinate(Point::new(u, v));
                let eval = |f: [f64; 3]| f[0] + f[1] * point.x + f[2] * point.y;
                assert!((eval(source.u) - u).abs() < 1e-14);
                assert!((eval(source.v) - v).abs() < 1e-14);
            }
        }
        patch.control_points[5].x = f64::from_bits(patch.control_points[5].x.to_bits() + 1);
        let rejected = PatchParameterIdentity::coons(&patch, 0, kurbo::Affine::IDENTITY).unwrap();
        assert!(!rejected.has_u() && !rejected.has_v());
    }
    fn owner(index: usize, color: [f64; 3]) -> PixelSourceOwner {
        PixelSourceOwner {
            patch_index: index,
            sample_index: 0,
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
    fn convex_piece_merge_never_fills_a_concave_union() {
        let admit = |_| Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>);
        let mut pixel = OrderedPixel::new(&admit, 64);
        let a = Point::new(0.25, 0.5);
        let b = Point::new(0.75, 0.5);
        for triangle in [[a, b, Point::new(0.25, 0.75)], [b, a, Point::ZERO]] {
            pixel
                .paint_triangle(triangle, owner(0, [1., 0., 0.]), &mut 1_000_000, &|| false)
                .unwrap();
        }
        close(integrated(&pixel), [0.1875, 0., 0., 0.1875]);
        assert!(pixel.fragments() >= 2);
    }

    #[test]
    fn losing_triangles_do_not_fragment_a_wholly_winning_owner() {
        let admit = |_| Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>);
        let mut pixel = OrderedPixel::new(&admit, 64);
        rectangle(&mut pixel, 0., 1., owner(2, [1., 0., 0.]));
        let fragments = pixel.fragments();
        let before = integrated(&pixel);
        for i in 1..32 {
            let x = i as f64 / 32.;
            pixel
                .paint_triangle(
                    [Point::new(x, 0.), Point::new(1., x), Point::new(0., 1.)],
                    owner(0, [0., 0., 1.]),
                    &mut 1_000_000,
                    &|| false,
                )
                .unwrap();
            assert_eq!(pixel.fragments(), fragments);
        }
        assert_eq!(integrated(&pixel), before);
    }

    #[test]
    fn wholly_occluded_triangles_need_no_new_admission_but_crossing_owners_do() {
        use std::cell::Cell;
        let deny = Cell::new(false);
        let calls = Cell::new(0usize);
        let admit = |_| {
            calls.set(calls.get() + 1);
            if deny.get() {
                None
            } else {
                Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>)
            }
        };
        let mut pixel = OrderedPixel::new(&admit, 64);
        let mut retained = owner(2, [1., 0., 0.]);
        retained.v = [0.75, 0., 0.];
        rectangle(&mut pixel, 0., 1., retained);
        let before = integrated(&pixel);
        let fragments = pixel.fragments();
        assert_eq!(fragments, 1);
        deny.set(true);
        calls.set(0);
        let triangle = [
            Point::new(0.25, 0.25),
            Point::new(0.75, 0.25),
            Point::new(0.25, 0.75),
        ];
        let mut lower = owner(2, [0., 0., 1.]);
        lower.v = [0.5, 0., 0.];
        pixel
            .paint_triangle(triangle, lower, &mut 2000, &|| false)
            .unwrap();
        pixel
            .paint_triangle(triangle, owner(1, [0., 0., 1.]), &mut 2000, &|| false)
            .unwrap();
        assert_eq!(calls.get(), 0);
        let checkpoints = Cell::new(0usize);
        pixel
            .paint_triangle(triangle, lower, &mut 2000, &|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            })
            .unwrap();
        assert!(checkpoints.get() > 10);
        for stop in 1..=checkpoints.get() {
            let at = Cell::new(0usize);
            assert_eq!(
                pixel.paint_triangle(triangle, lower, &mut 2000, &|| {
                    at.set(at.get() + 1);
                    at.get() == stop
                }),
                Err(PixelPartitionError::Cancelled)
            );
            assert_eq!(integrated(&pixel), before);
            assert_eq!(pixel.fragments(), fragments);
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(integrated(&pixel), before);
        assert_eq!(pixel.fragments(), fragments);
        assert_eq!(
            pixel.paint_triangle(triangle, lower, &mut 0, &|| false),
            Err(PixelPartitionError::Limit)
        );
        assert_eq!(
            pixel.paint_triangle(triangle, lower, &mut 2000, &|| true),
            Err(PixelPartitionError::Cancelled)
        );
        let mut tied = lower;
        tied.v = retained.v;
        assert_eq!(
            pixel.paint_triangle(triangle, tied, &mut 2000, &|| false),
            Err(PixelPartitionError::Admission)
        );
        let mut crossing = lower;
        crossing.v = [0., 2., 0.];
        assert_eq!(
            pixel.paint_triangle(triangle, crossing, &mut 2000, &|| false),
            Err(PixelPartitionError::Admission)
        );
        assert_eq!(
            pixel.paint_triangle(triangle, owner(3, [0., 0., 1.]), &mut 2000, &|| false),
            Err(PixelPartitionError::Admission)
        );
        assert_eq!(integrated(&pixel), before);
        deny.set(false);
        pixel
            .paint_triangle(triangle, crossing, &mut 100_000, &|| false)
            .unwrap();
        assert_ne!(integrated(&pixel), before);
        let mut ambiguous = OrderedPixel::new(&admit, 64);
        let mut tiny = owner(2, [1., 0., 0.]);
        tiny.v = [f64::from_bits(1), 0., 0.];
        rectangle(&mut ambiguous, 0., 1., tiny);
        let exact = integrated(&ambiguous);
        deny.set(true);
        assert_eq!(
            ambiguous.paint_triangle(triangle, owner(2, [0., 0., 1.]), &mut 2000, &|| false),
            Err(PixelPartitionError::Admission)
        );
        assert_eq!(integrated(&ambiguous), exact);
        deny.set(false);
        ambiguous
            .paint_triangle(triangle, owner(2, [0., 0., 1.]), &mut 100_000, &|| false)
            .unwrap();
        assert_eq!(integrated(&ambiguous), exact);
    }
    #[test]
    fn ordered_pixel_convex_clip_preserves_sampling_ids_and_shared_work() {
        let admit = |_| Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>);
        let mut pixel = OrderedPixel::new(&admit, 256);
        let mut a = owner(0, [1., 0., 0.]);
        a.v = [0., 1., 0.];
        a.sample_index = 11;
        let mut b = owner(0, [0., 0., 1.]);
        b.v = [1., -1., 0.];
        b.sample_index = 22;
        rectangle(&mut pixel, 0., 1., a);
        rectangle(&mut pixel, 0., 1., b);
        let clip = [Point::ZERO, Point::new(1., 0.), Point::new(0., 1.)];
        for points in [clip, [clip[2], clip[1], clip[0]]] {
            let mut areas = [0.; 2];
            let mut work = 1_000_000;
            pixel
                .visit_convex_clip(&points, &mut work, &|| false, &mut |area, _, _, source, work| {
                    *work = work.checked_sub(3).ok_or(PixelPartitionError::Limit)?;
                    match source.sample_index {
                        11 => areas[0] += area,
                        22 => areas[1] += area,
                        _ => panic!(),
                    };
                    Ok(())
                })
                .unwrap();
            assert!((areas[0] - 0.125).abs() < 1e-12 && (areas[1] - 0.375).abs() < 1e-12);
            assert!(work < 1_000_000);
        }
        let before = integrated(&pixel);
        assert_eq!(
            pixel
                .visit_convex_clip(&clip, &mut 0, &|| false, &mut |_, _, _, _, _| Ok(()))
                .unwrap_err(),
            PixelPartitionError::Limit
        );
        assert_eq!(
            pixel
                .visit_convex_clip(&clip, &mut 1_000_000, &|| true, &mut |_, _, _, _, _| Ok(()))
                .unwrap_err(),
            PixelPartitionError::Cancelled
        );
        assert_eq!(
            pixel
                .visit_convex_clip(&clip, &mut 1_000_000, &|| false, &mut |_, _, _, _, work| {
                    *work += 1;
                    Ok(())
                })
                .unwrap_err(),
            PixelPartitionError::Invalid
        );
        for points in [
            [
                Point::ZERO,
                Point::new(1., 0.),
                Point::new(0.25, 0.25),
                Point::new(0., 1.),
            ],
            [
                Point::ZERO,
                Point::new(1., 1.),
                Point::new(1., 0.),
                Point::new(0., 1.),
            ],
        ] {
            assert_eq!(
                pixel
                    .visit_convex_clip(&points, &mut 1_000_000, &|| false, &mut |_, _, _, _, _| Ok(()))
                    .unwrap_err(),
                PixelPartitionError::Invalid
            );
        }
        assert_eq!(integrated(&pixel), before);
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
