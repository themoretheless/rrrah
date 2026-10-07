#[cfg(test)]
mod tests {
    use hayro::hayro_interpret::shading::{AdaptivePatchError, AdaptivePatchLimits, TensorProductPatch};
    use kurbo::Affine;
    fn patch(curved: bool, cross_color: bool) -> TensorProductPatch {
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut control_points = TensorProductPatch {
            control_points: Default::default(),
            colors: std::array::from_fn(|_| Default::default()),
        }
        .control_points;
        for i in 0..4 {
            for j in 0..4 {
                control_points[indices[i][j]] = (
                    i as f64 * 100. / 3.,
                    j as f64 * 100. / 3. + if curved && (i == 1 || i == 2) { 12. } else { 0. },
                )
                    .into();
            }
        }
        let colors = if cross_color {
            [0., 0., 1., 0.]
        } else {
            [0., 0.5, 1., 0.5]
        };
        TensorProductPatch {
            control_points,
            colors: colors.map(|v| std::iter::once(v).collect()),
        }
    }
    fn limits() -> AdaptivePatchLimits {
        AdaptivePatchLimits {
            pixel_error: 0.1,
            component_error: 1. / 512.,
            max_depth: 12,
            max_triangles: 100_000,
        }
    }
    #[test]
    fn affine_patch_is_two_triangles_and_transform_controls_error() {
        let patch = patch(false, false);
        let mut output = vec![];
        let stats = patch
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut output, || false)
            .unwrap();
        assert_eq!(stats.triangles, 2);
        assert_eq!(stats.deepest, 0);
        assert!(stats.max_pixel_bound <= limits().pixel_error);
        assert!(stats.max_component_bound <= limits().component_error);
        let mut small = vec![];
        let mut large = vec![];
        let curved = patch_for_scale();
        let a = curved
            .to_triangles_adaptive(Affine::scale(0.1), limits(), &mut small, || false)
            .unwrap();
        let b = curved
            .to_triangles_adaptive(Affine::scale(10.), limits(), &mut large, || false)
            .unwrap();
        assert!(b.triangles > a.triangles);
    }
    fn patch_for_scale() -> TensorProductPatch {
        patch(true, false)
    }
    #[test]
    fn dense_curved_geometry_and_bilinear_color_stay_within_reported_bounds() {
        let patch = patch(true, true);
        let mut output = vec![];
        let stats = patch
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut output, || false)
            .unwrap();
        assert!(stats.triangles > 2);
        assert!(stats.max_pixel_bound <= limits().pixel_error);
        assert!(stats.max_component_bound <= limits().component_error);
        for triangle in &output {
            let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
            let uv = vertices.map(|vertex| {
                let u = vertex.point.x / 100.;
                let v = (vertex.point.y - patch.map_coordinate((u, 0.).into()).y) / 100.;
                kurbo::Point::new(u, v)
            });
            for i in 0..9 {
                for j in 0..9 - i {
                    let weights = [i as f64 / 8., j as f64 / 8., 1. - (i + j) as f64 / 8.];
                    let point = uv
                        .iter()
                        .zip(weights)
                        .fold(kurbo::Vec2::ZERO, |p, (uv, w)| p + uv.to_vec2() * w)
                        .to_point();
                    let approximate = vertices
                        .iter()
                        .zip(weights)
                        .fold(kurbo::Vec2::ZERO, |p, (vertex, w)| p + vertex.point.to_vec2() * w);
                    assert!(
                        (patch.map_coordinate(point).to_vec2() - approximate).hypot()
                            <= stats.max_pixel_bound + 1e-10
                    );
                    let color = vertices
                        .iter()
                        .zip(weights)
                        .map(|(vertex, w)| f64::from(vertex.colors[0]) * w)
                        .sum::<f64>();
                    assert!(
                        (f64::from(patch.interpolate(point)[0]) - color).abs()
                            <= stats.max_component_bound + 1e-8
                    );
                }
            }
        }
    }
    #[test]
    fn errors_and_cancellation_roll_back_only_new_triangles() {
        let base = patch(false, false);
        let mut output = vec![];
        base.to_triangles_adaptive(Affine::IDENTITY, limits(), &mut output, || false)
            .unwrap();
        let original = output[0].p0.point;
        let curved = patch(true, true);
        let mut tight = limits();
        tight.max_triangles = 3;
        assert_eq!(
            curved
                .to_triangles_adaptive(Affine::IDENTITY, tight, &mut output, || false)
                .unwrap_err(),
            AdaptivePatchError::TriangleLimit
        );
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].p0.point, original);
        tight = limits();
        tight.max_depth = 0;
        assert_eq!(
            curved
                .to_triangles_adaptive(Affine::IDENTITY, tight, &mut output, || false)
                .unwrap_err(),
            AdaptivePatchError::Depth
        );
        assert_eq!(output.len(), 2);
        let mut polls = 0;
        assert_eq!(
            curved
                .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut output, || {
                    polls += 1;
                    polls == 12
                })
                .unwrap_err(),
            AdaptivePatchError::Cancelled
        );
        assert_eq!(output.len(), 2);
        let mut invalid = limits();
        invalid.pixel_error = f64::NAN;
        assert_eq!(
            base.to_triangles_adaptive(Affine::IDENTITY, invalid, &mut output, || false)
                .unwrap_err(),
            AdaptivePatchError::Invalid
        );
        assert_eq!(
            base.to_triangles_adaptive(Affine::IDENTITY, invalid, &mut output, || true)
                .unwrap_err(),
            AdaptivePatchError::Cancelled
        );
    }
    #[test]
    fn coons_control_conversion_matches_tensor_geometry_and_colors() {
        use hayro::hayro_interpret::shading::CoonsPatch;
        let tensor = patch(true, true);
        let coons = CoonsPatch {
            control_points: std::array::from_fn(|i| tensor.control_points[i]),
            colors: tensor.colors.clone(),
        };
        for i in 0..33 {
            for j in 0..33 {
                let p = (i as f64 / 32., j as f64 / 32.).into();
                assert!((coons.map_coordinate(p) - tensor.map_coordinate(p)).hypot() < 1e-10);
            }
        }
        let mut a = vec![];
        let mut b = vec![];
        let ta = tensor
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut a, || false)
            .unwrap();
        let tb = coons
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut b, || false)
            .unwrap();
        assert_eq!(ta.triangles, tb.triangles);
        for (a, b) in a.iter().zip(&b) {
            for (a, b) in [&a.p0, &a.p1, &a.p2].into_iter().zip([&b.p0, &b.p1, &b.p2]) {
                assert!((a.point - b.point).hypot() < 1e-10);
                assert_eq!(a.colors, b.colors);
            }
        }
    }
    #[test]
    fn adaptive_cells_share_complete_edges_without_t_junctions() {
        let mut patch = patch(false, false);
        patch.control_points[12].y += 30.;
        let mut triangles = vec![];
        patch
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut triangles, || false)
            .unwrap();
        let uv = |point: kurbo::Point| {
            let u = point.x / 100.;
            let mut lo = 0.;
            let mut hi = 1.;
            for _ in 0..50 {
                let mid = (lo + hi) * 0.5;
                if patch.map_coordinate((u, mid).into()).y < point.y {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (
                (u * 65536.).round() as i32,
                (((lo + hi) * 0.5) * 65536.).round() as i32,
            )
        };
        let mut edges = std::collections::BTreeMap::new();
        let mut total_twice_area = 0i64;
        for triangle in &triangles {
            let points = [triangle.p0.point, triangle.p1.point, triangle.p2.point];
            let q = points.map(uv);
            let area = i64::from(q[1].0 - q[0].0) * i64::from(q[2].1 - q[0].1)
                - i64::from(q[1].1 - q[0].1) * i64::from(q[2].0 - q[0].0);
            assert!(area > 0);
            total_twice_area += area;
            for (a, b) in [
                (points[0], points[1]),
                (points[1], points[2]),
                (points[2], points[0]),
            ] {
                let bits = |p: kurbo::Point| (p.x.to_bits(), p.y.to_bits());
                let (a, b) = if bits(a) < bits(b) { (a, b) } else { (b, a) };
                let entry = edges.entry((bits(a), bits(b))).or_insert((0, uv(a), uv(b)));
                entry.0 += 1;
            }
        }
        assert_eq!(total_twice_area, 2 * 65536i64 * 65536);
        for (_, (count, a, b)) in edges {
            let boundary =
                (a.0 == b.0 && [0, 65536].contains(&a.0)) || (a.1 == b.1 && [0, 65536].contains(&a.1));
            assert_eq!(
                count,
                if boundary { 1 } else { 2 },
                "unshared or duplicated geometric edge {a:?}--{b:?}"
            );
        }
        let vertices: std::collections::BTreeSet<_> = triangles
            .iter()
            .flat_map(|t| [uv(t.p0.point), uv(t.p1.point), uv(t.p2.point)])
            .collect();
        for t in &triangles {
            let p = [uv(t.p0.point), uv(t.p1.point), uv(t.p2.point)];
            for (a, b) in [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])] {
                if a.0 != b.0 && a.1 != b.1 {
                    continue;
                }
                for &v in &vertices {
                    let inside = if a.0 == b.0 {
                        v.0 == a.0 && v.1 > a.1.min(b.1) && v.1 < a.1.max(b.1)
                    } else {
                        v.1 == a.1 && v.0 > a.0.min(b.0) && v.0 < a.0.max(b.0)
                    };
                    assert!(!inside, "T junction: edge {a:?}--{b:?}, unmatched vertex {v:?}");
                }
            }
        }
    }
}
