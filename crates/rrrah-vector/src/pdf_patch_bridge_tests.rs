#[cfg(test)]
mod tests {
    use hayro::hayro_interpret::{
        encode::{OrderedPixel, PatchParameterIdentity, PixelPartitionError, PixelSourceOwner},
        shading::{
            AdaptivePatchLimits, AdaptivePatchRef, TensorProductPatch,
            tessellate_patch_mesh_stream_ordered_admitted, tessellate_patch_mesh_stream_projected_admitted,
        },
    };
    use kurbo::{Affine, Point};
    fn patch(xs: [f64; 4], color: [f32; 3]) -> TensorProductPatch {
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut controls = [Point::ZERO; 16];
        for i in 0..4 {
            for j in 0..4 {
                controls[indices[i][j]] = Point::new(xs[i], j as f64);
            }
        }
        TensorProductPatch {
            control_points: controls,
            colors: std::array::from_fn(|_| color.into_iter().collect()),
        }
    }
    fn limits() -> AdaptivePatchLimits {
        AdaptivePatchLimits {
            pixel_error: 1. / 64.,
            component_error: 1. / 1024.,
            max_depth: 12,
            max_triangles: 1_048_576,
        }
    }
    #[test]
    fn patch_bridge_reconstructs_reflected_transformed_parameters_and_components() {
        let mut source = patch([0., 0.5, 1., 1.5], [0.; 3]);
        source.colors =
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]].map(|v| v.into_iter().collect());
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let transform = Affine::new([-1., 0.25, 0.5, 1., 11., 17.]);
        let origin = Point::new(10., 16.);
        let mut count = 0;
        tessellate_patch_mesh_stream_ordered_admitted(
            &[AdaptivePatchRef::Tensor(&source)],
            transform,
            limits(),
            &|| false,
            &admit,
            &mut |triangle, provenance| {
                let owner = PixelSourceOwner::from_triangle(&triangle, provenance, origin).unwrap();
                assert_eq!(owner.patch_index, 0);
                for (vertex, uv) in [&triangle.p0, &triangle.p1, &triangle.p2]
                    .into_iter()
                    .zip(provenance.parameters)
                {
                    let point = Point::new(vertex.point.x - origin.x, vertex.point.y - origin.y);
                    let values = owner.evaluate(point).unwrap();
                    for i in 0..3 {
                        assert!((values[i] - f64::from(vertex.colors[i])).abs() < 1e-12);
                    }
                    for (field, expected) in [(owner.u, uv.x), (owner.v, uv.y)] {
                        assert!(
                            (field[0] + field[1] * point.x + field[2] * point.y - expected).abs() < 1e-12
                        );
                    }
                }
                let mut invalid = triangle.clone();
                invalid.p2.point = invalid.p1.point;
                assert_eq!(
                    PixelSourceOwner::from_triangle(&invalid, provenance, origin).unwrap_err(),
                    PixelPartitionError::Conditioning
                );
                invalid.p2.point = Point::new(f64::NAN, 0.);
                assert_eq!(
                    PixelSourceOwner::from_triangle(&invalid, provenance, origin).unwrap_err(),
                    PixelPartitionError::Invalid
                );
                count += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(root.used(), 0);
    }
    fn raster(sources: &[TensorProductPatch]) -> Vec<u8> {
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut pixels = (0..9)
            .map(|_| OrderedPixel::new(&admit, 4096))
            .collect::<Vec<_>>();
        let refs = sources.iter().map(AdaptivePatchRef::Tensor).collect::<Vec<_>>();
        let identities = sources
            .iter()
            .enumerate()
            .map(|(i, p)| PatchParameterIdentity::tensor(p, i, Affine::IDENTITY).unwrap())
            .collect::<Vec<_>>();
        let mut work = 100_000_000;
        let mut max_v_field_error = 0.0f64;
        let stats = tessellate_patch_mesh_stream_projected_admitted(
            &refs,
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |triangle, provenance| {
                let bbox = triangle.bounding_box();
                for y in 0..3 {
                    for x in 0..3 {
                        if bbox.x1 <= x as f64
                            || bbox.y1 <= y as f64
                            || bbox.x0 >= (x + 1) as f64
                            || bbox.y0 >= (y + 1) as f64
                        {
                            continue;
                        }
                        let origin = Point::new(x as f64, y as f64);
                        let mut owner = PixelSourceOwner::from_triangle(&triangle, provenance, origin).unwrap();
                        identities[provenance.patch_index].apply(&mut owner,origin).unwrap();
                        // This helper's control nets all have source y=3v.
                        max_v_field_error = max_v_field_error.max((owner.v[0]-origin.y/3.).abs())
                            .max(owner.v[1].abs()).max((owner.v[2]-1./3.).abs());
                        let points = [&triangle.p0, &triangle.p1, &triangle.p2]
                            .map(|v| Point::new(v.point.x - origin.x, v.point.y - origin.y));
                        pixels[y * 3 + x]
                            .paint_triangle(points, owner, &mut work, &|| false)
                            .unwrap_or_else(|error| panic!("{error:?}: pixel={x},{y} fragments={} used={} work_left={work} provenance={provenance:?}",pixels[y*3+x].fragments(),root.used()));
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(stats.max_component_bound <= limits().component_error);
        let mut result = Vec::new();
        let mut fragments = 0;
        for pixel in &pixels {
            let mut sum = [0.; 4];
            fragments += pixel.fragments();
            pixel
                .visit(&mut work, &|| false, &mut |area, center, _, owner| {
                    let values = owner.evaluate(center).unwrap();
                    for i in 0..3 {
                        sum[i] += values[i] * area;
                    }
                    sum[3] += area;
                    Ok(())
                })
                .unwrap();
            if sum[3] > 1. + 1e-10 {
                if let Some(prefix) = std::env::var_os("RRRAH_ORDERED_PIXEL_TRACE") {
                    let mut regions = Vec::new();
                    pixel
                        .visit(&mut 1_000_000, &|| false, &mut |area, center, polygon, owner| {
                            regions.push(serde_json::json!({"area":area,"centroid":[center.x,center.y],
                            "polygon":polygon.iter().map(|p|[p.x,p.y]).collect::<Vec<_>>(),
                            "patch":owner.patch_index,"u":owner.u,"v":owner.v}));
                            Ok(())
                        })
                        .unwrap();
                    let path = format!(
                        "{}-{}.json",
                        std::path::Path::new(&prefix).display(),
                        sources[0].colors[2][0]
                    );
                    std::fs::write(path, serde_json::to_vec_pretty(&regions).unwrap()).unwrap();
                }
            }
            assert!(
                sum[3] <= 1. + 1e-10,
                "sum={sum:?} fragments={}",
                pixel.fragments()
            );
            for value in &sum[..3] {
                result.push(if sum[3] > 0. {
                    ((*value / sum[3]) as f32 * 255.).round() as u8
                } else {
                    0
                });
            }
            // Match the renderer's existing f32 sample/coverage contract.
            result.push((sum[3] as f32 * 255.).round() as u8);
        }
        eprintln!(
            "PATCH_BRIDGE triangles={} fragments={fragments} peak={} work_left={work} max_v_field_error={max_v_field_error}",
            stats.triangles,
            root.peak()
        );
        drop(pixels);
        assert_eq!(root.used(), 0);
        result
    }
    #[test]
    fn patch_bridge_partial_overlap_matches_exact_ordered_three_pixel_rows() {
        let red = patch([0., 0.5, 1., 1.5], [1., 0., 0.]);
        let blue = patch([0.5, 1., 1.5, 2.], [0., 0., 1.]);
        assert_eq!(
            raster(&[red, blue]),
            [128, 0, 128, 255, 0, 0, 255, 255, 0, 0, 0, 0].repeat(3)
        );
    }
    #[test]
    fn patch_bridge_folded_projection_matches_exact_union_three_pixel_rows() {
        let folded = patch([0., 2., 2., 0.], [0.; 3]);
        assert_eq!(
            raster(&[folded]),
            [0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 0].repeat(3)
        );
    }
    #[test]
    fn patch_bridge_colored_fold_selects_largest_u_mapping() {
        let mut folded = patch([0., 2., 2., 0.], [0.; 3]);
        folded.colors =
            [[0., 0., 0.], [0., 0., 0.], [1., 1., 1.], [1., 1., 1.]].map(|v| v.into_iter().collect());
        // Greatest-u inverse is (1+sqrt(1-2x/3))/2. Exact source integrals:
        // first pixel 1-(1/3)^(3/2)/2; second .25+(1/3)^(3/2)/2,
        // with the latter unpremultiplied by its half-pixel coverage.
        let power = (1.0f64 / 3.).powf(1.5);
        let a = ((1. - power / 2.) * 255.).round() as u8;
        let b = ((0.25 + power / 2.) * 2. * 255.).round() as u8;
        assert_eq!((a, b), (230, 177));
        assert_eq!(
            raster(&[folded]),
            [a, a, a, 255, b, b, b, 128, 0, 0, 0, 0].repeat(3)
        );
    }
    #[test]
    fn patch_identity_proves_global_relations_and_rejects_one_ulp_curve_change() {
        let folded = patch([0., 2., 2., 0.], [0.; 3]);
        let identity = PatchParameterIdentity::tensor(&folded, 0, Affine::IDENTITY).unwrap();
        assert!(!identity.has_u() && identity.has_v());
        let mut perturbed = folded.clone();
        perturbed.control_points[12].y = perturbed.control_points[12].y.next_up();
        let changed = PatchParameterIdentity::tensor(&perturbed, 0, Affine::IDENTITY).unwrap();
        assert!(
            !changed.has_u() && !changed.has_v(),
            "one-ulp deviations cannot be rounded into a certificate"
        );
        let mut planar = patch([0., 0.5, 1., 1.5], [0.; 3]);
        for p in &mut planar.control_points {
            let (x, y) = (p.x, p.y);
            *p = Point::new(x + 2. * y, 3. * x - y);
        }
        let transform = Affine::new([-1., 0.25, 0.5, 1., 11., 17.]);
        let identity = PatchParameterIdentity::tensor(&planar, 7, transform).unwrap();
        assert!(identity.has_u() && identity.has_v());
        let origin = Point::new(9., 13.);
        let mut owner = PixelSourceOwner {
            patch_index: 7,
            sample_index: 0,
            u: [77.; 3],
            v: [88.; 3],
            components: [[0.; 3]; 4],
            component_count: 1,
        };
        identity.apply(&mut owner, origin).unwrap();
        for uv in [Point::new(0., 0.), Point::new(0.25, 0.75), Point::new(1., 1.)] {
            let p = transform * planar.map_coordinate(uv);
            let p = Point::new(p.x - origin.x, p.y - origin.y);
            for (field, expected) in [(owner.u, uv.x), (owner.v, uv.y)] {
                assert!((field[0] + field[1] * p.x + field[2] * p.y - expected).abs() < 1e-12);
            }
        }
    }
    #[test]
    fn patch_identity_overflow_and_invalid_composition_never_mutate_owner() {
        let source = patch([0., 0.5, 1., 1.5], [0.; 3]);
        assert!(PatchParameterIdentity::tensor(&source, 0, Affine::scale(0.)).is_none());
        let mut huge = source.clone();
        huge.control_points[0].x = f64::MAX;
        assert!(PatchParameterIdentity::tensor(&huge, 0, Affine::IDENTITY).is_none());
        let mut overflow_device = source.clone();
        for p in &mut overflow_device.control_points {
            p.x *= 2f64.powi(700);
            p.y *= 2f64.powi(700);
        }
        assert!(PatchParameterIdentity::tensor(&overflow_device, 0, Affine::scale(1e150)).is_none());
        huge.control_points[0].x = f64::NAN;
        assert!(PatchParameterIdentity::tensor(&huge, 0, Affine::IDENTITY).is_none());
        let identity = PatchParameterIdentity::tensor(&source, 0, Affine::scale(0.1)).unwrap();
        let mut owner = PixelSourceOwner {
            patch_index: 0,
            sample_index: 0,
            u: [77.; 3],
            v: [88.; 3],
            components: [[0.; 3]; 4],
            component_count: 1,
        };
        let before = (owner.u, owner.v);
        assert_eq!(
            identity
                .apply(&mut owner, Point::new(f64::MAX, f64::MAX))
                .unwrap_err(),
            PixelPartitionError::Conditioning
        );
        assert_eq!((owner.u, owner.v), before);
        owner.patch_index = 1;
        assert_eq!(
            identity.apply(&mut owner, Point::ZERO).unwrap_err(),
            PixelPartitionError::Invalid
        );
        assert_eq!((owner.u, owner.v), before);
    }
    #[test]
    fn patch_identity_preserves_folded_parameter_under_integer_affine_families() {
        let mut checked = 0;
        for n in 0..64 {
            let a = (n % 7 + 1) as f64;
            let b = ((n / 7) % 5) as f64 - 2.;
            let c = ((n / 35) % 3) as f64 - 1.;
            let d = (2 + n % 5) as f64;
            if a * d - b * c == 0. {
                continue;
            }
            let mut source = patch([0., 2., 2., 0.], [0.; 3]);
            for p in &mut source.control_points {
                let (x, y) = (p.x, p.y);
                *p = Point::new(a * x + b * y + 1000., c * x + d * y + 2000.);
            }
            let identity = PatchParameterIdentity::tensor(&source, 0, Affine::IDENTITY).unwrap();
            assert!(!identity.has_u() && identity.has_v(), "affine family {n}");
            let origin = Point::new(1000., 2000.);
            let mut owner = PixelSourceOwner {
                patch_index: 0,
                sample_index: 0,
                u: [77.; 3],
                v: [88.; 3],
                components: [[0.; 3]; 4],
                component_count: 1,
            };
            identity.apply(&mut owner, origin).unwrap();
            assert_eq!(owner.u, [77.; 3], "unproved u must remain untouched");
            for uv in [
                Point::new(0.25, 0.25),
                Point::new(0.75, 0.75),
                Point::new(0.5, 0.5),
            ] {
                let point = source.map_coordinate(uv);
                let point = Point::new(point.x - origin.x, point.y - origin.y);
                assert!(
                    (owner.v[0] + owner.v[1] * point.x + owner.v[2] * point.y - uv.y).abs() < 1e-10,
                    "family {n}"
                );
            }
            let mut changed = source.clone();
            if c != 0. {
                changed.control_points[12].x = changed.control_points[12].x.next_up();
            } else {
                changed.control_points[12].y = changed.control_points[12].y.next_up();
            }
            assert!(
                !PatchParameterIdentity::tensor(&changed, 0, Affine::IDENTITY)
                    .unwrap()
                    .has_v(),
                "ulp family {n}"
            );
            checked += 1;
        }
        assert!(checked >= 50);
    }
    #[test]
    fn projected_bounds_detect_curved_projection_and_enclose_source_residuals() {
        use hayro::hayro_interpret::encode::projected_component_bounds;
        let mut source = patch([0., 2., 2., 0.], [0.; 3]);
        source.colors =
            [[0., 0., 0.], [0., 0., 0.], [1., 1., 1.], [1., 1., 1.]].map(|v| v.into_iter().collect());
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut maximum = 0.0f64;
        let mut seen = 0;
        tessellate_patch_mesh_stream_ordered_admitted(
            &[AdaptivePatchRef::Tensor(&source)],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |triangle, provenance| {
                let bound = projected_component_bounds(
                    &source,
                    Affine::IDENTITY,
                    &triangle,
                    provenance,
                    &mut 4096,
                    &|| false,
                )
                .unwrap();
                maximum = maximum.max(bound[0]);
                let origin = triangle.p0.point;
                let owner = PixelSourceOwner::from_triangle(&triangle, provenance, origin).unwrap();
                for (a, b) in [(0.2, 0.2), (0.6, 0.2), (0.2, 0.6)] {
                    let uv = Point::new(
                        (1. - a - b) * provenance.parameters[0].x
                            + a * provenance.parameters[1].x
                            + b * provenance.parameters[2].x,
                        (1. - a - b) * provenance.parameters[0].y
                            + a * provenance.parameters[1].y
                            + b * provenance.parameters[2].y,
                    );
                    let p = source.map_coordinate(uv);
                    let local = Point::new(p.x - origin.x, p.y - origin.y);
                    let actual = owner.evaluate(local).unwrap();
                    for k in 0..3 {
                        assert!((actual[k] - uv.x).abs() <= bound[k] + 1e-12);
                    }
                }
                if seen == 0 {
                    assert_eq!(
                        projected_component_bounds(
                            &source,
                            Affine::IDENTITY,
                            &triangle,
                            provenance,
                            &mut 0,
                            &|| false
                        )
                        .unwrap_err(),
                        PixelPartitionError::Limit
                    );
                    assert_eq!(
                        projected_component_bounds(
                            &source,
                            Affine::IDENTITY,
                            &triangle,
                            provenance,
                            &mut 4096,
                            &|| true
                        )
                        .unwrap_err(),
                        PixelPartitionError::Cancelled
                    );
                }
                seen += 1;
                Ok(())
            },
        )
        .unwrap();
        assert!(seen > 2 && maximum > limits().component_error);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn projected_stream_depth_refusal_is_terminal_and_releases_credit() {
        use hayro::hayro_interpret::shading::AdaptivePatchError;
        let mut source = patch([0., 0.25, 0.75, 1.5], [0.; 3]);
        source.colors =
            [[0., 0., 0.], [0., 0., 0.], [1., 1., 1.], [1., 1., 1.]].map(|v| v.into_iter().collect());
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut constrained = limits();
        constrained.pixel_error = 1000.;
        constrained.max_depth = 0;
        assert_eq!(
            tessellate_patch_mesh_stream_projected_admitted(
                &[AdaptivePatchRef::Tensor(&source)],
                Affine::IDENTITY,
                constrained,
                &|| false,
                &admit,
                &mut |_, _| panic!("unqualified components must not reach consumer")
            )
            .unwrap_err(),
            AdaptivePatchError::Depth
        );
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn projected_stream_coons_preserves_limits_cancel_admission_failure_and_retry() {
        use hayro::hayro_interpret::shading::{AdaptivePatchError, CoonsPatch};
        use std::cell::Cell;
        let mut tensor = patch([0., 0.5, 1., 1.5], [0.; 3]);
        tensor.colors =
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]].map(|v| v.into_iter().collect());
        let coons = CoonsPatch {
            control_points: std::array::from_fn(|i| tensor.control_points[i]),
            colors: tensor.colors.clone(),
        };
        let refs = [AdaptivePatchRef::Coons(&coons)];
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
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
        let mut seen = 0;
        let stats = tessellate_patch_mesh_stream_projected_admitted(
            &refs,
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |_, source| {
                assert_eq!(source.patch_index, 0);
                seen += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(seen, 2);
        assert!(stats.max_component_bound <= limits().component_error);
        let allocations = calls.get();
        assert!(allocations > 0);
        assert_eq!(root.used(), 0);
        for n in 1..=allocations {
            calls.set(0);
            refuse.set(n);
            assert_eq!(
                tessellate_patch_mesh_stream_projected_admitted(
                    &refs,
                    Affine::IDENTITY,
                    limits(),
                    &|| false,
                    &admit,
                    &mut |_, _| Ok(())
                )
                .unwrap_err(),
                AdaptivePatchError::Admission
            );
            assert_eq!(root.used(), 0);
        }
        refuse.set(usize::MAX);
        assert_eq!(
            tessellate_patch_mesh_stream_projected_admitted(
                &refs,
                Affine::IDENTITY,
                limits(),
                &|| false,
                &admit,
                &mut |_, _| Err(AdaptivePatchError::Admission)
            )
            .unwrap_err(),
            AdaptivePatchError::Admission
        );
        assert_eq!(root.used(), 0);
        let cancelled = Cell::new(false);
        assert_eq!(
            tessellate_patch_mesh_stream_projected_admitted(
                &refs,
                Affine::IDENTITY,
                limits(),
                &|| cancelled.get(),
                &admit,
                &mut |_, _| {
                    cancelled.set(true);
                    Ok(())
                }
            )
            .unwrap_err(),
            AdaptivePatchError::Cancelled
        );
        assert_eq!(root.used(), 0);
        let mut tiny = limits();
        tiny.max_triangles = 1;
        assert_eq!(
            tessellate_patch_mesh_stream_projected_admitted(
                &refs,
                Affine::IDENTITY,
                tiny,
                &|| false,
                &admit,
                &mut |_, _| panic!("triangle limit cannot publish")
            )
            .unwrap_err(),
            AdaptivePatchError::TriangleLimit
        );
        assert_eq!(root.used(), 0);
        tessellate_patch_mesh_stream_projected_admitted(
            &refs,
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(root.used(), 0);
        let mut curved = patch([0., 2., 2., 0.], [0.; 3]);
        curved.colors =
            [[0., 0., 0.], [0., 0., 0.], [1., 1., 1.], [1., 1., 1.]].map(|v| v.into_iter().collect());
        let curved = CoonsPatch {
            control_points: std::array::from_fn(|i| curved.control_points[i]),
            colors: curved.colors,
        };
        let mut curved_count = 0;
        let stats = tessellate_patch_mesh_stream_projected_admitted(
            &[AdaptivePatchRef::Coons(&curved)],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |_, _| {
                curved_count += 1;
                Ok(())
            },
        )
        .unwrap();
        assert!(curved_count > 2 && stats.max_component_bound <= limits().component_error);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn projected_stream_anisotropic_refinement_keeps_uv_edges_conforming() {
        use std::collections::{BTreeMap, BTreeSet};
        use std::ops::Bound::Excluded;
        let mut source = patch([0., 2., 2., 0.], [0.; 3]);
        source.colors =
            [[0., 0., 0.], [0., 0., 0.], [1., 1., 1.], [1., 1., 1.]].map(|v| v.into_iter().collect());
        let root = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        let admit = |bytes: usize| {
            root.try_reserve(bytes as u64)
                .ok()
                .map(|r| Box::new(r) as Box<dyn std::any::Any + Send + Sync>)
        };
        let mut vertical = BTreeMap::<u64, BTreeSet<u64>>::new();
        let mut horizontal = BTreeMap::<u64, BTreeSet<u64>>::new();
        let mut positions = BTreeMap::new();
        let mut edges = Vec::new();
        let stats = tessellate_patch_mesh_stream_projected_admitted(
            &[AdaptivePatchRef::Tensor(&source)],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |triangle, meta| {
                for (uv, vertex) in
                    meta.parameters
                        .into_iter()
                        .zip([&triangle.p0, &triangle.p1, &triangle.p2])
                {
                    vertical.entry(uv.x.to_bits()).or_default().insert(uv.y.to_bits());
                    horizontal
                        .entry(uv.y.to_bits())
                        .or_default()
                        .insert(uv.x.to_bits());
                    let point = (vertex.point.x.to_bits(), vertex.point.y.to_bits());
                    if let Some(old) = positions.insert((uv.x.to_bits(), uv.y.to_bits()), point) {
                        assert_eq!(old, point);
                    }
                }
                for i in 0..3 {
                    let a = meta.parameters[i];
                    let b = meta.parameters[(i + 1) % 3];
                    if a.x == b.x && a.y != b.y {
                        edges.push((
                            true,
                            a.x.to_bits(),
                            a.y.min(b.y).to_bits(),
                            a.y.max(b.y).to_bits(),
                        ));
                    }
                    if a.y == b.y && a.x != b.x {
                        edges.push((
                            false,
                            a.y.to_bits(),
                            a.x.min(b.x).to_bits(),
                            a.x.max(b.x).to_bits(),
                        ));
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(stats.triangles > 2 && !edges.is_empty());
        for (vertical_edge, fixed, low, high) in edges {
            let values = if vertical_edge {
                &vertical[&fixed]
            } else {
                &horizontal[&fixed]
            };
            assert!(
                values.range((Excluded(low), Excluded(high))).next().is_none(),
                "T-junction along UV cell edge"
            );
        }
        assert_eq!(root.used(), 0);
    }
}
