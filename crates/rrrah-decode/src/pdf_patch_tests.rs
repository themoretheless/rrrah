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
    fn coons_conversion_preserves_source_boundary_bits() {
        use hayro::hayro_interpret::shading::CoonsPatch;
        let mut tensor = patch(false, false);
        // Fractional coordinates expose cancellation in the general Coons formula.
        for (index, point) in tensor.control_points.iter_mut().enumerate().take(12) {
            point.x += (index as f64 + 1.) * 0.0135792468;
            point.y += (index as f64 + 1.) * 0.0314159265;
        }
        tensor.colors =
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]].map(|components| components.into_iter().collect());
        let coons = CoonsPatch {
            control_points: std::array::from_fn(|i| tensor.control_points[i]),
            colors: tensor.colors.clone(),
        };
        let mut triangles = vec![];
        coons
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut triangles, || false)
            .unwrap();
        let mut checked = 0;
        for triangle in &triangles {
            for vertex in [&triangle.p0, &triangle.p1, &triangle.p2] {
                let u = f64::from(vertex.colors[0]);
                let v = f64::from(vertex.colors[1]);
                if u == 0. || u == 1. || v == 0. || v == 1. {
                    let expected = tensor.map_coordinate((u, v).into());
                    assert_eq!(vertex.point.x.to_bits(), expected.x.to_bits(), "x at {u},{v}");
                    assert_eq!(vertex.point.y.to_bits(), expected.y.to_bits(), "y at {u},{v}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 8, "must cover subdivided source boundaries");
    }
    #[test]
    fn mesh_stitches_unequal_depths_and_reversed_edges_without_blending_colors() {
        use hayro::hayro_interpret::shading::tessellate_tensor_patch_mesh_adaptive;
        use std::collections::BTreeSet;
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let edge_segments = |triangles: &[hayro::hayro_interpret::shading::Triangle], color: f32| {
            let mut result = BTreeSet::new();
            for triangle in triangles {
                let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
                if vertices[0].colors[0] != color {
                    continue;
                }
                assert!(vertices.iter().all(|v| v.colors[0] == color));
                for i in 0..3 {
                    let (a, b) = (vertices[i], vertices[(i + 1) % 3]);
                    if a.point.x == 100. && b.point.x == 100. {
                        let mut ends = [a.point.y.to_bits(), b.point.y.to_bits()];
                        ends.sort();
                        result.insert(ends);
                    }
                }
            }
            result
        };
        for reversed in [false, true] {
            let mut left = patch(false, false);
            left.control_points[15].y += 30.;
            left.control_points[8].y += 0.75;
            left.control_points[7].y -= 0.375;
            left.colors = std::array::from_fn(|_| std::iter::once(1.).collect());
            let mut right = patch(false, false);
            let original = right.control_points;
            for i in 0..4 {
                for j in 0..4 {
                    let mut point = original[indices[i][if reversed { 3 - j } else { j }]];
                    point.x += 100.;
                    right.control_points[indices[i][j]] = point;
                }
            }
            // Copy the source edge exactly, rather than constructing an approximate match.
            for j in 0..4 {
                right.control_points[indices[0][j]] =
                    left.control_points[indices[3][if reversed { 3 - j } else { j }]];
            }
            right.colors = std::array::from_fn(|_| std::iter::once(2.).collect());
            let patches = [left, right];
            let mut independent = vec![];
            for patch in &patches {
                patch
                    .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut independent, || false)
                    .unwrap();
            }
            assert_ne!(edge_segments(&independent, 1.), edge_segments(&independent, 2.));
            let mut triangles = vec![];
            let mut polls = 0;
            let stats = tessellate_tensor_patch_mesh_adaptive(
                &patches,
                Affine::IDENTITY,
                limits(),
                &mut triangles,
                || {
                    polls += 1;
                    false
                },
            )
            .unwrap();
            let a = edge_segments(&triangles, 1.);
            assert!(a.len() > 1);
            assert_eq!(a, edge_segments(&triangles, 2.), "reversed={reversed}");
            // The converted Coons interior differs from the tensor source;
            // matching only the boundary is sufficient for a conforming mesh.
            use hayro::hayro_interpret::shading::{
                AdaptivePatchRef, CoonsPatch, tessellate_patch_mesh_adaptive,
            };
            let coons = CoonsPatch {
                control_points: std::array::from_fn(|i| patches[1].control_points[i]),
                colors: patches[1].colors.clone(),
            };
            for refs in [
                [
                    AdaptivePatchRef::Tensor(&patches[0]),
                    AdaptivePatchRef::Coons(&coons),
                ],
                [
                    AdaptivePatchRef::Coons(&coons),
                    AdaptivePatchRef::Tensor(&patches[0]),
                ],
            ] {
                let first_color = match refs[0] {
                    AdaptivePatchRef::Tensor(_) => 1.,
                    _ => 2.,
                };
                let mut mixed = vec![];
                let mut mixed_polls = 0;
                let stats =
                    tessellate_patch_mesh_adaptive(&refs, Affine::IDENTITY, limits(), &mut mixed, || {
                        mixed_polls += 1;
                        false
                    })
                    .unwrap();
                assert_eq!(stats.triangles, mixed.len());
                assert_eq!(mixed[0].p0.colors[0], first_color);
                assert!(edge_segments(&mixed, 1.).len() > 1);
                assert_eq!(
                    edge_segments(&mixed, 1.),
                    edge_segments(&mixed, 2.),
                    "mixed reversed={reversed}"
                );
                assert!(stats.max_pixel_bound <= limits().pixel_error);
                assert!(stats.max_component_bound <= limits().component_error);
                let sentinel = mixed[0].clone();
                let mut kept = vec![sentinel.clone()];
                let mut short = limits();
                short.max_triangles = stats.triangles - 1;
                assert_eq!(
                    tessellate_patch_mesh_adaptive(&refs, Affine::IDENTITY, short, &mut kept, || false)
                        .unwrap_err(),
                    AdaptivePatchError::TriangleLimit
                );
                assert_eq!(kept.len(), 1);
                let mut seen = 0;
                assert_eq!(
                    tessellate_patch_mesh_adaptive(&refs, Affine::IDENTITY, limits(), &mut kept, || {
                        seen += 1;
                        seen == mixed_polls - 1
                    })
                    .unwrap_err(),
                    AdaptivePatchError::Cancelled
                );
                assert_eq!(kept.len(), 1);
                assert_eq!(kept[0].p0.point, sentinel.p0.point);
            }
            assert_eq!(stats.triangles, triangles.len());
            assert!(stats.max_pixel_bound <= limits().pixel_error);
            assert!(stats.max_component_bound <= limits().component_error);
            let sentinel = triangles[0].clone();
            let mut preserved = vec![sentinel.clone()];
            let mut short = limits();
            short.max_triangles = stats.triangles - 1;
            assert_eq!(
                tessellate_tensor_patch_mesh_adaptive(
                    &patches,
                    Affine::IDENTITY,
                    short,
                    &mut preserved,
                    || false
                )
                .unwrap_err(),
                AdaptivePatchError::TriangleLimit
            );
            assert_eq!(preserved.len(), 1);
            assert_eq!(preserved[0].p0.point, sentinel.p0.point);
            let mut seen = 0;
            assert_eq!(
                tessellate_tensor_patch_mesh_adaptive(
                    &patches,
                    Affine::IDENTITY,
                    limits(),
                    &mut preserved,
                    || {
                        seen += 1;
                        seen == polls - 1
                    }
                )
                .unwrap_err(),
                AdaptivePatchError::Cancelled
            );
            assert_eq!(preserved.len(), 1);
            assert_eq!(preserved[0].p0.point, sentinel.p0.point);
        }
    }
    #[test]
    fn mesh_does_not_weld_near_curves_and_validates_empty_requests() {
        use hayro::hayro_interpret::shading::tessellate_tensor_patch_mesh_adaptive;
        let mut left = patch(false, false);
        left.control_points[15].y += 30.;
        let mut right = patch(false, false);
        for point in &mut right.control_points {
            point.x += 100.;
        }
        right.control_points[1].x += 1e-9;
        let patches = [left, right];
        let mut independent = vec![];
        for patch in &patches {
            patch
                .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut independent, || false)
                .unwrap();
        }
        let mut mesh = vec![];
        tessellate_tensor_patch_mesh_adaptive(&patches, Affine::IDENTITY, limits(), &mut mesh, || false)
            .unwrap();
        assert_eq!(
            mesh.len(),
            independent.len(),
            "distinct control curves must not share subdivision points"
        );
        assert_eq!(
            tessellate_tensor_patch_mesh_adaptive(&[], Affine::IDENTITY, limits(), &mut mesh, || false)
                .unwrap()
                .triangles,
            0
        );
        let mut invalid = limits();
        invalid.pixel_error = f64::NAN;
        assert_eq!(
            tessellate_tensor_patch_mesh_adaptive(&[], Affine::IDENTITY, invalid, &mut mesh, || false)
                .unwrap_err(),
            AdaptivePatchError::Invalid
        );
        assert_eq!(
            tessellate_tensor_patch_mesh_adaptive(&[], Affine::IDENTITY, invalid, &mut mesh, || true)
                .unwrap_err(),
            AdaptivePatchError::Cancelled
        );
        assert_eq!(mesh.len(), independent.len());
    }
    #[test]
    fn mixed_mesh_ranges_preserve_patch_ownership_and_roll_back_together() {
        use hayro::hayro_interpret::shading::{
            AdaptivePatchRef, CoonsPatch, tessellate_patch_mesh_adaptive_with_ranges,
        };
        let mut tensor = patch(true, false);
        tensor.colors = std::array::from_fn(|_| std::iter::once(1.).collect());
        let coons = CoonsPatch {
            control_points: std::array::from_fn(|i| tensor.control_points[i]),
            colors: std::array::from_fn(|_| std::iter::once(2.).collect()),
        };
        let refs = [AdaptivePatchRef::Tensor(&tensor), AdaptivePatchRef::Coons(&coons)];
        let mut prefix = vec![];
        patch(false, false)
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut prefix, || false)
            .unwrap();
        let mut output = vec![prefix[0].clone()];
        let mut ranges = vec![0..1];
        let mut polls = 0;
        let stats = tessellate_patch_mesh_adaptive_with_ranges(
            &refs,
            Affine::IDENTITY,
            limits(),
            &mut output,
            &mut ranges,
            || {
                polls += 1;
                false
            },
        )
        .unwrap();
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], 0..1);
        assert_eq!(ranges[1].start, 1);
        assert_eq!(ranges[1].end, ranges[2].start);
        assert_eq!(ranges[2].end, output.len());
        assert_eq!(stats.triangles, ranges[1].len() + ranges[2].len());
        for (range, color) in ranges[1..].iter().zip([1., 2.]) {
            assert!(!range.is_empty());
            for triangle in &output[range.clone()] {
                assert!(
                    [&triangle.p0, &triangle.p1, &triangle.p2]
                        .iter()
                        .all(|v| v.colors[0] == color)
                );
            }
        }
        let original_ranges = ranges.clone();
        let original_len = output.len();
        let mut short = limits();
        short.max_triangles = stats.triangles - 1;
        assert_eq!(
            tessellate_patch_mesh_adaptive_with_ranges(
                &refs,
                Affine::IDENTITY,
                short,
                &mut output,
                &mut ranges,
                || false
            )
            .unwrap_err(),
            AdaptivePatchError::TriangleLimit
        );
        assert_eq!(output.len(), original_len);
        assert_eq!(ranges, original_ranges);
        let mut seen = 0;
        assert_eq!(
            tessellate_patch_mesh_adaptive_with_ranges(
                &refs,
                Affine::IDENTITY,
                limits(),
                &mut output,
                &mut ranges,
                || {
                    seen += 1;
                    seen == polls - 1
                }
            )
            .unwrap_err(),
            AdaptivePatchError::Cancelled
        );
        assert_eq!(output.len(), original_len);
        assert_eq!(ranges, original_ranges);
        assert_eq!(
            tessellate_patch_mesh_adaptive_with_ranges(
                &[],
                Affine::IDENTITY,
                limits(),
                &mut output,
                &mut ranges,
                || false
            )
            .unwrap()
            .triangles,
            0
        );
        assert_eq!(output.len(), original_len);
        assert_eq!(ranges, original_ranges);
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
