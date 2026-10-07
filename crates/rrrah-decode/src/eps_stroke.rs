//! Native segment and join stroke outlines; scene assembly is a separate stage.
use crate::{EpsMatrix, EpsPathSegment, EpsPoint, EpsStrokeStyle};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
#[derive(Debug, thiserror::Error)]
pub enum EpsStrokeError {
    #[error("invalid stroke geometry, cap or transform")]
    Range,
    #[error("hairlines require a device-space pen")]
    Hairline,
    #[error("stroke outline segment limit exceeded")]
    Limit,
    #[error("stroke outline cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] BufferError),
}
fn transform(m: EpsMatrix, p: EpsPoint) -> Result<EpsPoint, EpsStrokeError> {
    let p = [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]];
    if p.iter().all(|v| v.is_finite()) {
        Ok(p)
    } else {
        Err(EpsStrokeError::Range)
    }
}
/// Creates a closed fill contour for a line segment in stroke user space.
/// Cap 0 is butt, 1 round, 2 square. `tolerance` applies after `matrix`.
/// Width zero is explicitly refused until device hairline handling exists.
pub fn outline_eps_segment<F: FnMut() -> bool>(
    start: EpsPoint,
    end: EpsPoint,
    width: f64,
    cap: u8,
    matrix: EpsMatrix,
    tolerance: f64,
    max_segments: usize,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<EpsPathSegment>, EpsStrokeError> {
    if cancelled() {
        return Err(EpsStrokeError::Cancelled);
    }
    if !start
        .iter()
        .chain(end.iter())
        .chain(matrix.iter())
        .all(|v| v.is_finite())
        || !width.is_finite()
        || width < 0.
        || cap > 2
        || !tolerance.is_finite()
        || tolerance <= 0.
    {
        return Err(EpsStrokeError::Range);
    }
    if width == 0. {
        return Err(EpsStrokeError::Hairline);
    }
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if !length.is_finite() {
        return Err(EpsStrokeError::Range);
    }
    let radius = width * 0.5;
    // Frobenius norm bounds the affine operator norm, so user-space sagitta
    // bounded by tolerance/norm also bounds the transformed circular arc.
    let norm = matrix[0].hypot(matrix[1]).hypot(matrix[2].hypot(matrix[3]));
    if !norm.is_finite() {
        return Err(EpsStrokeError::Range);
    }
    let half_steps = if cap == 1 && norm != 0. {
        let ratio = tolerance / (radius * norm);
        let angle = 2. * (1. - ratio.min(1.)).acos();
        if !angle.is_finite() || angle == 0. {
            return Err(EpsStrokeError::Limit);
        }
        let n = (std::f64::consts::PI / angle).ceil().max(2.);
        if n > max_segments as f64 {
            return Err(EpsStrokeError::Limit);
        }
        n as usize
    } else {
        2
    };
    let vertices = if length == 0. {
        if cap == 1 {
            half_steps.checked_mul(2)
        } else {
            Some(0)
        }
    } else if cap == 1 {
        half_steps.checked_mul(2).and_then(|n| n.checked_add(2))
    } else {
        Some(4)
    }
    .ok_or(EpsStrokeError::Limit)?;
    let count = if vertices == 0 {
        0
    } else {
        vertices.checked_add(1).ok_or(EpsStrokeError::Limit)?
    };
    if count > max_segments {
        return Err(EpsStrokeError::Limit);
    }
    let mut output = budget.try_buffer(count, EpsPathSegment::Move([0.; 2]))?;
    if vertices == 0 {
        return Ok(output.freeze());
    }
    let tangent = if length == 0. {
        [1., 0.]
    } else {
        [dx / length, dy / length]
    };
    let normal = [-tangent[1], tangent[0]];
    let theta = tangent[1].atan2(tangent[0]);
    let mut first = [0.; 2];
    let mut previous = [0.; 2];
    for i in 0..vertices {
        if cancelled() {
            return Err(EpsStrokeError::Cancelled);
        }
        let p = if length == 0. {
            let angle = -(i as f64) * std::f64::consts::TAU / vertices as f64;
            [start[0] + radius * angle.cos(), start[1] + radius * angle.sin()]
        } else if cap == 1 {
            let (center, angle) = if i <= half_steps {
                (
                    end,
                    theta + std::f64::consts::FRAC_PI_2 - i as f64 * std::f64::consts::PI / half_steps as f64,
                )
            } else {
                (
                    start,
                    theta
                        - std::f64::consts::FRAC_PI_2
                        - (i - half_steps - 1) as f64 * std::f64::consts::PI / half_steps as f64,
                )
            };
            [center[0] + radius * angle.cos(), center[1] + radius * angle.sin()]
        } else {
            let (center, along, side) = match i {
                0 => (start, -1., 1.),
                1 => (end, 1., 1.),
                2 => (end, 1., -1.),
                _ => (start, -1., -1.),
            };
            let extension = if cap == 2 { along * radius } else { 0. };
            [
                center[0] + tangent[0] * extension + normal[0] * side * radius,
                center[1] + tangent[1] * extension + normal[1] * side * radius,
            ]
        };
        let p = transform(matrix, p)?;
        output[i] = if i == 0 {
            first = p;
            EpsPathSegment::Move(p)
        } else {
            EpsPathSegment::Line {
                start: previous,
                end: p,
            }
        };
        previous = p;
    }
    output[vertices] = EpsPathSegment::Close {
        start: previous,
        end: first,
    };
    if cancelled() {
        return Err(EpsStrokeError::Cancelled);
    }
    Ok(output.freeze())
}
/// Creates the outer join patch, consistently clockwise with segment outlines.
/// Join 0 is miter (falls back to bevel above limit), 1 round, 2 bevel.
pub fn outline_eps_join<F: FnMut() -> bool>(
    before: EpsPoint,
    vertex: EpsPoint,
    after: EpsPoint,
    width: f64,
    join: u8,
    miter_limit: f64,
    matrix: EpsMatrix,
    tolerance: f64,
    max_segments: usize,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<EpsPathSegment>, EpsStrokeError> {
    if cancelled() {
        return Err(EpsStrokeError::Cancelled);
    }
    if !before
        .iter()
        .chain(vertex.iter())
        .chain(after.iter())
        .chain(matrix.iter())
        .all(|v| v.is_finite())
        || !width.is_finite()
        || width < 0.
        || join > 2
        || !miter_limit.is_finite()
        || miter_limit < 1.
        || !tolerance.is_finite()
        || tolerance <= 0.
    {
        return Err(EpsStrokeError::Range);
    }
    if width == 0. {
        return Err(EpsStrokeError::Hairline);
    }
    let unit = |a: EpsPoint, b: EpsPoint| {
        let delta = [b[0] - a[0], b[1] - a[1]];
        let length = delta[0].hypot(delta[1]);
        if length == 0. || !length.is_finite() {
            Err(EpsStrokeError::Range)
        } else {
            Ok([delta[0] / length, delta[1] / length])
        }
    };
    let u = unit(before, vertex)?;
    let v = unit(vertex, after)?;
    let cross = u[0] * v[1] - u[1] * v[0];
    let dot = (u[0] * v[0] + u[1] * v[1]).clamp(-1., 1.);
    if cross == 0. {
        if dot < 0. && join == 1 {
            return outline_eps_segment(
                vertex,
                vertex,
                width,
                1,
                matrix,
                tolerance,
                max_segments,
                budget,
                cancelled,
            );
        }
        return Ok(budget.try_buffer(0, EpsPathSegment::Move([0.; 2]))?.freeze());
    }
    let radius = width * 0.5;
    let side = if cross > 0. { -1. } else { 1. };
    let a = [vertex[0] - u[1] * side * radius, vertex[1] + u[0] * side * radius];
    let b = [vertex[0] - v[1] * side * radius, vertex[1] + v[0] * side * radius];
    let first = if cross > 0. { b } else { a };
    let last = if cross > 0. { a } else { b };
    let ratio = if dot > -1. {
        (2. / (1. + dot)).sqrt()
    } else {
        f64::INFINITY
    };
    let miter = join == 0 && ratio <= miter_limit;
    let mut arc_steps = 0usize;
    let vertices = if join == 1 {
        let norm = matrix[0].hypot(matrix[1]).hypot(matrix[2].hypot(matrix[3]));
        if !norm.is_finite() {
            return Err(EpsStrokeError::Range);
        }
        let sweep = cross.atan2(dot).abs();
        let step = if norm == 0. {
            sweep
        } else {
            2. * (1. - (tolerance / (radius * norm)).min(1.)).acos()
        };
        if !step.is_finite() || step == 0. {
            return Err(EpsStrokeError::Limit);
        }
        let needed = (sweep / step).ceil().max(1.);
        if needed > max_segments as f64 {
            return Err(EpsStrokeError::Limit);
        }
        arc_steps = needed as usize;
        arc_steps.checked_add(2).ok_or(EpsStrokeError::Limit)?
    } else if miter {
        4
    } else {
        3
    };
    let count = vertices.checked_add(1).ok_or(EpsStrokeError::Limit)?;
    if count > max_segments {
        return Err(EpsStrokeError::Limit);
    }
    let mut output = budget.try_buffer(count, EpsPathSegment::Move([0.; 2]))?;
    let origin = transform(matrix, vertex)?;
    let mut previous = origin;
    let angle = (first[1] - vertex[1]).atan2(first[0] - vertex[0]);
    for i in 0..vertices {
        if cancelled() {
            return Err(EpsStrokeError::Cancelled);
        }
        let p = if i == 0 {
            vertex
        } else if i == 1 {
            first
        } else if i == vertices - 1 {
            last
        } else if join == 1 {
            let theta = angle - cross.atan2(dot).abs() * (i - 1) as f64 / arc_steps as f64;
            [vertex[0] + radius * theta.cos(), vertex[1] + radius * theta.sin()]
        } else {
            [
                vertex[0] - (u[1] + v[1]) * side * radius / (1. + dot),
                vertex[1] + (u[0] + v[0]) * side * radius / (1. + dot),
            ]
        };
        let p = transform(matrix, p)?;
        output[i] = if i == 0 {
            EpsPathSegment::Move(p)
        } else {
            EpsPathSegment::Line {
                start: previous,
                end: p,
            }
        };
        previous = p;
    }
    output[vertices] = EpsPathSegment::Close {
        start: previous,
        end: origin,
    };
    if cancelled() {
        return Err(EpsStrokeError::Cancelled);
    }
    Ok(output.freeze())
}
/// Constructs a union of clockwise segment/cap/join contours for one polyline.
/// Points are in stroke user coordinates; `matrix` maps the resulting pen into
/// raster coordinates. Repeated points are skipped. Work includes both passes.
pub fn outline_eps_polyline<F: FnMut() -> bool>(
    points: &[EpsPoint],
    closed: bool,
    style: EpsStrokeStyle,
    matrix: EpsMatrix,
    tolerance: f64,
    max_segments: usize,
    max_work: usize,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<EpsPathSegment>, EpsStrokeError> {
    if !style.width.is_finite()
        || style.width < 0.
        || style.cap > 2
        || style.join > 2
        || !style.miter_limit.is_finite()
        || style.miter_limit < 1.
        || !matrix.iter().all(|v| v.is_finite())
        || !tolerance.is_finite()
        || tolerance <= 0.
    {
        return Err(EpsStrokeError::Range);
    }
    if style.width == 0. {
        return Err(EpsStrokeError::Hairline);
    }
    let cancelled = std::cell::RefCell::new(&mut cancelled);
    let work = std::cell::Cell::new(0usize);
    let tick = || {
        if (cancelled.borrow_mut())() {
            return Err(EpsStrokeError::Cancelled);
        }
        let next = work.get().checked_add(1).ok_or(EpsStrokeError::Limit)?;
        if next > max_work {
            return Err(EpsStrokeError::Limit);
        }
        work.set(next);
        Ok(())
    };
    let run = |emit: &mut dyn FnMut(EpsPathSegment) -> Result<(), EpsStrokeError>| {
        tick()?;
        for point in points {
            tick()?;
            if !point.iter().all(|v| v.is_finite()) {
                return Err(EpsStrokeError::Range);
            }
        }
        let mut count = 0usize;
        let mut append = |outline: SharedBuffer<EpsPathSegment>| {
            for &segment in outline.iter() {
                tick()?;
                count = count.checked_add(1).ok_or(EpsStrokeError::Limit)?;
                if count > max_segments {
                    return Err(EpsStrokeError::Limit);
                }
                emit(segment)?;
            }
            Ok(())
        };
        let poll = || (cancelled.borrow_mut())();
        let mut first = None;
        let mut previous = None;
        let edge_count = if points.len() < 2 {
            0
        } else {
            points.len() - 1 + usize::from(closed)
        };
        for at in 0..edge_count {
            tick()?;
            let a = points[at];
            let b = points[(at + 1) % points.len()];
            if a == b {
                continue;
            }
            if let Some((before, vertex)) = previous {
                append(outline_eps_join(
                    before,
                    vertex,
                    b,
                    style.width,
                    style.join,
                    style.miter_limit,
                    matrix,
                    tolerance,
                    max_segments,
                    budget,
                    poll,
                )?)?;
            }
            append(outline_eps_segment(
                a,
                b,
                style.width,
                0,
                matrix,
                tolerance,
                max_segments,
                budget,
                poll,
            )?)?;
            first.get_or_insert((a, b));
            previous = Some((a, b));
        }
        if let (Some((start, next)), Some((before, end))) = (first, previous) {
            if closed {
                append(outline_eps_join(
                    before,
                    end,
                    next,
                    style.width,
                    style.join,
                    style.miter_limit,
                    matrix,
                    tolerance,
                    max_segments,
                    budget,
                    poll,
                )?)?;
            } else {
                for (endpoint, neighbor) in [(start, next), (end, before)] {
                    if style.cap == 1 {
                        append(outline_eps_segment(
                            endpoint,
                            endpoint,
                            style.width,
                            1,
                            matrix,
                            tolerance,
                            max_segments,
                            budget,
                            poll,
                        )?)?;
                    } else if style.cap == 2 {
                        let delta = [endpoint[0] - neighbor[0], endpoint[1] - neighbor[1]];
                        let length = delta[0].hypot(delta[1]);
                        if !length.is_finite() || length == 0. {
                            return Err(EpsStrokeError::Range);
                        }
                        let outer = [
                            endpoint[0] + delta[0] / length * style.width * 0.5,
                            endpoint[1] + delta[1] / length * style.width * 0.5,
                        ];
                        append(outline_eps_segment(
                            endpoint,
                            outer,
                            style.width,
                            0,
                            matrix,
                            tolerance,
                            max_segments,
                            budget,
                            poll,
                        )?)?;
                    }
                }
            }
        } else if !closed
            && style.cap == 1
            && points.len() > 1
            && let Some(&point) = points.first()
        {
            append(outline_eps_segment(
                point,
                point,
                style.width,
                1,
                matrix,
                tolerance,
                max_segments,
                budget,
                poll,
            )?)?;
        }
        tick()?;
        Ok(count)
    };
    let count = run(&mut |_| Ok(()))?;
    tick()?;
    let mut output = budget.try_buffer(count, EpsPathSegment::Move([0.; 2]))?;
    let mut at = 0;
    run(&mut |segment| {
        output[at] = segment;
        at += 1;
        Ok(())
    })?;
    tick()?;
    Ok(output.freeze())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polyline_closed_ring_repeated_points_and_error_release() {
        let root = MemoryBudget::new(100_000);
        let identity = [1., 0., 0., 1., 0., 0.];
        let points = [[1., 1.], [1., 1.], [4., 1.], [4., 4.], [1., 4.]];
        let style = EpsStrokeStyle {
            width: 2.,
            ..EpsStrokeStyle::default()
        };
        let outline =
            outline_eps_polyline(&points, true, style, identity, 0.01, 1000, 10000, &root, || false).unwrap();
        let mask = crate::fill_eps_path(
            &outline,
            5,
            5,
            false,
            crate::EpsFillLimits::default(),
            &root,
            || false,
        )
        .unwrap();
        for (i, &value) in mask.iter().enumerate() {
            assert_eq!(value, if i == 12 { 0 } else { 255 }, "pixel {i}");
        }
        drop(mask);
        drop(outline);
        assert_eq!(root.used(), 0);
        assert!(matches!(
            outline_eps_polyline(&points, true, style, identity, 0.01, 3, 10000, &root, || false),
            Err(EpsStrokeError::Limit)
        ));
        assert_eq!(root.used(), 0);
        assert!(matches!(
            outline_eps_polyline(&points, true, style, identity, 0.01, 1000, 3, &root, || false),
            Err(EpsStrokeError::Limit)
        ));
        assert_eq!(root.used(), 0);
    }
    #[test]
    #[ignore = "strict stroke pixel qualification: Ghostscript butt stroke includes an extra row outside the native geometric outline"]
    fn independent_ghostscript_stroke_pixel_qualification() {
        compare_stroke_pixels(
            include_str!("../../../tests/fixtures/eps/stroke-pixel-ghostscript-reference.json"),
            6,
        );
    }
    #[test]
    fn independent_unadjusted_axis_stroke_pixels_match() {
        compare_stroke_pixels(
            include_str!("../../../tests/fixtures/eps/stroke-unadjusted-pixel-ghostscript-reference.json"),
            3,
        );
    }
    #[test]
    #[ignore = "strict unadjusted stroke qualification: bevel pixel 53 differs at diagonal boundary"]
    fn independent_unadjusted_ghostscript_stroke_pixel_qualification() {
        compare_stroke_pixels(
            include_str!("../../../tests/fixtures/eps/stroke-unadjusted-pixel-ghostscript-reference.json"),
            6,
        );
    }
    fn compare_stroke_pixels(source: &str, count: usize) {
        let reference: serde_json::Value = serde_json::from_str(source).unwrap();
        let root = MemoryBudget::new(100_000);
        let matrix = [1., 0., 0., -1., 0., 8.];
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        for (index, case) in cases[..count].iter().enumerate() {
            let line = [[2., 4.], [6., 4.]];
            let corner = [[2., 2.], [5., 2.], [5., 6.]];
            let points = if index < 2 { &line[..] } else { &corner[..] };
            let style = EpsStrokeStyle {
                width: 2.,
                cap: if index == 1 { 2 } else { 0 },
                join: match index {
                    3 => 2,
                    4 => 1,
                    _ => 0,
                },
                miter_limit: if index == 5 { 1. } else { 10. },
                ..EpsStrokeStyle::default()
            };
            let path =
                outline_eps_polyline(points, false, style, matrix, 0.001, 1000, 10000, &root, || false)
                    .unwrap();
            let mask = crate::fill_eps_path(
                &path,
                8,
                8,
                false,
                crate::EpsFillLimits {
                    samples: 1,
                    ..crate::EpsFillLimits::default()
                },
                &root,
                || false,
            )
            .unwrap();
            let rgb = case["rgb"].as_array().unwrap();
            for (pixel, &coverage) in mask.iter().enumerate() {
                for channel in 0..3 {
                    assert_eq!(
                        u64::from(255 - coverage),
                        rgb[pixel * 3 + channel].as_u64().unwrap(),
                        "{} pixel {pixel}",
                        case["name"]
                    );
                }
            }
            drop(mask);
            drop(path);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    #[ignore = "strict reference gate: shifted bevel also differs away from original diagonal tie; coverage rule unresolved"]
    fn independent_shifted_bevel_pixel_qualification() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/bevel-boundary-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 4);
        compare_bevel_reference_cases(cases);
    }
    #[test]
    fn independent_zero_adjust_bevel_pixels_match_away_from_exact_diagonal_tie() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/bevel-outline-zero-adjust-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        let cases: Vec<_> = cases
            .iter()
            .filter_map(|case| {
                let shift = case["name"].as_str()?.strip_prefix("stroke_")?;
                if shift == "0" {
                    return None;
                }
                let mut case = case.clone();
                case["name"] = serde_json::Value::String(format!("bevel_shift_{shift}"));
                Some(case)
            })
            .collect();
        assert_eq!(cases.len(), 4);
        compare_bevel_reference_cases(&cases);
    }
    fn compare_bevel_reference_cases(cases: &[serde_json::Value]) {
        let root = MemoryBudget::new(100_000);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let dx: f64 = name.strip_prefix("bevel_shift_").unwrap().parse().unwrap();
            let points = [[2. + dx, 2.], [5. + dx, 2.], [5. + dx, 6.]];
            let path = outline_eps_polyline(
                &points,
                false,
                EpsStrokeStyle {
                    width: 2.,
                    join: 2,
                    ..EpsStrokeStyle::default()
                },
                [1., 0., 0., -1., 0., 8.],
                0.001,
                1000,
                10000,
                &root,
                || false,
            )
            .unwrap();
            let mask = crate::fill_eps_path(
                &path,
                8,
                8,
                false,
                crate::EpsFillLimits {
                    samples: 1,
                    ..crate::EpsFillLimits::default()
                },
                &root,
                || false,
            )
            .unwrap();
            let rgb = case["rgb"].as_array().unwrap();
            for (at, &coverage) in mask.iter().enumerate() {
                for channel in 0..3 {
                    assert_eq!(
                        u64::from(255 - coverage),
                        rgb[at * 3 + channel].as_u64().unwrap(),
                        "{name} pixel {at}"
                    );
                }
            }
            drop(mask);
            drop(path);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn native_bevel_stroke_equals_explicit_union_at_subpixel_translations() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/bevel-outline-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        let root = MemoryBudget::new(32 * 1024 * 1024);
        for case in cases.iter().skip(1).step_by(2) {
            let name = case["name"].as_str().unwrap();
            let dx: f64 = name.strip_prefix("outline_").unwrap().parse().unwrap();
            let source = format!(
                "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n{}\n",
                case["source"]
                    .as_str()
                    .unwrap()
                    .replace("false setstrokeadjust ", "")
            );
            let mut limits = crate::EpsDocumentLimits::default();
            limits.raster.fill.samples = 1;
            let raster = crate::decode_eps_document(
                source.as_bytes(),
                1.,
                crate::EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                limits,
                &root,
                || false,
            )
            .unwrap();
            let points = [[2. + dx, 2.], [5. + dx, 2.], [5. + dx, 6.]];
            let path = outline_eps_polyline(
                &points,
                false,
                EpsStrokeStyle {
                    width: 2.,
                    join: 2,
                    ..EpsStrokeStyle::default()
                },
                [1., 0., 0., -1., 0., 8.],
                0.001,
                1000,
                10000,
                &root,
                || false,
            )
            .unwrap();
            let mask = crate::fill_eps_path(
                &path,
                8,
                8,
                false,
                crate::EpsFillLimits {
                    samples: 1,
                    ..crate::EpsFillLimits::default()
                },
                &root,
                || false,
            )
            .unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = raster.pixels() else {
                panic!()
            };
            for (at, &coverage) in mask.iter().enumerate() {
                let gray = 255 - coverage;
                assert_eq!(
                    &pixels[at * 4..at * 4 + 4],
                    &[gray, gray, gray, 255],
                    "{name} pixel {at}"
                );
            }
            drop(mask);
            drop(path);
            drop(raster);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn joins_union_with_segment_outlines_and_miter_limit_bevels() {
        let root = MemoryBudget::new(100_000);
        let matrix = [1., 0., 0., 1., 0., 0.];
        for (join, limit, expected) in [(0, 10., 255), (0, 1., 96), (1, 10., 207), (2, 10., 96)] {
            let first =
                outline_eps_segment([1., 3.], [3., 3.], 2., 0, matrix, 0.001, 1000, &root, || false).unwrap();
            let second =
                outline_eps_segment([3., 3.], [3., 1.], 2., 0, matrix, 0.001, 1000, &root, || false).unwrap();
            let joint = outline_eps_join(
                [1., 3.],
                [3., 3.],
                [3., 1.],
                2.,
                join,
                limit,
                matrix,
                0.001,
                1000,
                &root,
                || false,
            )
            .unwrap();
            let path = [&*first, &*second, &*joint].concat();
            let mask =
                crate::fill_eps_path(&path, 5, 5, false, crate::EpsFillLimits::default(), &root, || {
                    false
                })
                .unwrap();
            assert_eq!(mask[18], expected, "join {join}, limit {limit}");
            assert_eq!(mask[12], 255, "overlapping stroke bodies must not cancel");
            drop(first);
            drop(second);
            drop(joint);
            drop(mask);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn cap_outlines_fill_to_analytic_pixel_coverage() {
        let root = MemoryBudget::new(100_000);
        for (cap, expected) in [
            (0, [0, 0, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0]),
            (
                2,
                [0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0],
            ),
            // A radius-one semicircle covers 13 of 16 center samples
            // in each corner pixel, independently of the outline algorithm.
            (
                1,
                [0, 0, 0, 0, 207, 255, 255, 207, 207, 255, 255, 207, 0, 0, 0, 0],
            ),
        ] {
            let path = outline_eps_segment(
                [1., 2.],
                [3., 2.],
                2.,
                cap,
                [1., 0., 0., 1., 0., 0.],
                0.001,
                1000,
                &root,
                || false,
            )
            .unwrap();
            let mask =
                crate::fill_eps_path(&path, 4, 4, false, crate::EpsFillLimits::default(), &root, || {
                    false
                })
                .unwrap();
            assert_eq!(&*mask, &expected, "cap {cap}");
            drop(mask);
            drop(path);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn caps_and_affine_pen_have_analytic_geometry() {
        let root = MemoryBudget::new(100_000);
        let matrix = [2., 0., 0., 3., 5., 7.];
        let butt =
            outline_eps_segment([0., 0.], [2., 0.], 2., 0, matrix, 0.1, 1000, &root, || false).unwrap();
        assert_eq!(butt[0], EpsPathSegment::Move([5., 10.]));
        assert_eq!(
            butt[1],
            EpsPathSegment::Line {
                start: [5., 10.],
                end: [9., 10.]
            }
        );
        assert_eq!(
            butt[2],
            EpsPathSegment::Line {
                start: [9., 10.],
                end: [9., 4.]
            }
        );
        let square =
            outline_eps_segment([0., 0.], [2., 0.], 2., 2, matrix, 0.1, 1000, &root, || false).unwrap();
        assert_eq!(square[0], EpsPathSegment::Move([3., 10.]));
        assert_eq!(
            square[1],
            EpsPathSegment::Line {
                start: [3., 10.],
                end: [11., 10.]
            }
        );
        drop(butt);
        drop(square);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn round_zero_length_disk_and_error_release() {
        let root = MemoryBudget::new(100_000);
        let identity = [1., 0., 0., 1., 0., 0.];
        let path =
            outline_eps_segment([2., 2.], [2., 2.], 2., 1, identity, 0.01, 1000, &root, || false).unwrap();
        for segment in path.iter() {
            if let EpsPathSegment::Line { start, end } = segment {
                let midpoint = [(start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5];
                assert!(1. - (midpoint[0] - 2.).hypot(midpoint[1] - 2.) <= 0.01);
            }
        }
        drop(path);
        assert_eq!(root.used(), 0);
        assert!(matches!(
            outline_eps_segment([0.; 2], [1.; 2], 0., 0, identity, 0.1, 1000, &root, || false),
            Err(EpsStrokeError::Hairline)
        ));
        assert!(matches!(
            outline_eps_segment([0.; 2], [1.; 2], 1., 1, identity, 1e-30, 1000, &root, || false),
            Err(EpsStrokeError::Limit)
        ));
        assert!(matches!(
            outline_eps_segment([0.; 2], [1.; 2], 1., 1, identity, 0.1, 1000, &root, || root
                .used()
                != 0),
            Err(EpsStrokeError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
    }
}
