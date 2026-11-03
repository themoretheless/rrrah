//! Bounded user-space dash subdivision. Output retains vertices inside each dash.
use crate::{EpsPathSegment as S, EpsPoint, EpsStrokeError};
use rrrah_core::{MemoryBudget, SharedBuffer};
/// Maximum admitted dash entries. Odd arrays repeat twice to restore parity.
pub const EPS_MAX_DASH_ITEMS: usize = 32;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpsDashPattern {
    items: [f64; EPS_MAX_DASH_ITEMS],
    length: usize,
    offset: f64,
    pub(crate) device_matrix: Option<crate::EpsMatrix>,
    pub(crate) array_reference: Option<(usize, usize)>,
}
impl Default for EpsDashPattern {
    fn default() -> Self {
        Self {
            items: [0.; EPS_MAX_DASH_ITEMS],
            length: 0,
            offset: 0.,
            device_matrix: None,
            array_reference: None,
        }
    }
}
impl EpsDashPattern {
    pub fn new(items: &[f64], offset: f64) -> Result<Self, EpsStrokeError> {
        if items.len() > EPS_MAX_DASH_ITEMS {
            return Err(EpsStrokeError::Limit);
        }
        if !offset.is_finite() || items.iter().any(|x| !x.is_finite() || *x < 0.) {
            return Err(EpsStrokeError::Range);
        }
        let sum: f64 = items.iter().sum();
        if !items.is_empty()
            && (sum <= 0.
                || !sum.is_finite()
                || !(sum * if items.len() % 2 == 0 { 1. } else { 2. }).is_finite())
        {
            return Err(EpsStrokeError::Range);
        }
        let mut result = Self::default();
        result.items[..items.len()].copy_from_slice(items);
        result.length = items.len();
        result.offset = offset;
        Ok(result)
    }
    pub fn items(&self) -> &[f64] {
        &self.items[..self.length]
    }
    pub fn offset(&self) -> f64 {
        self.offset
    }
    pub fn is_solid(&self) -> bool {
        self.length == 0
    }
}
/// Split one polyline into independent on-runs. Corners inside a run retain their
/// original vertices. Closed seam runs are joined, rather than capped separately.
/// Both counting and writing passes share the work/cancellation budget.
pub fn dash_eps_polyline<F: FnMut() -> bool>(
    points: &[EpsPoint],
    closed: bool,
    pattern: EpsDashPattern,
    max_segments: usize,
    max_work: usize,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<SharedBuffer<S>, EpsStrokeError> {
    subdivide_dash_polyline(
        points,
        closed,
        pattern,
        max_segments,
        max_work,
        budget,
        cancelled,
        None,
    )
}
/// Device policy for butt-capped zero dash fragments. Other caps retain their
/// geometric zero length and are outlined by the existing cap primitive.
pub(crate) fn dash_eps_polyline_for_stroke<F: FnMut() -> bool>(
    points: &[EpsPoint],
    closed: bool,
    pattern: EpsDashPattern,
    matrix: crate::EpsMatrix,
    cap: u8,
    max_segments: usize,
    max_work: usize,
    budget: &MemoryBudget,
    cancelled: F,
) -> Result<SharedBuffer<S>, EpsStrokeError> {
    subdivide_dash_polyline(
        points,
        closed,
        pattern,
        max_segments,
        max_work,
        budget,
        cancelled,
        if cap == 0 { Some(matrix) } else { None },
    )
}
fn visible_dash_endpoint(
    point: EpsPoint,
    direction: EpsPoint,
    matrix: crate::EpsMatrix,
) -> Result<EpsPoint, EpsStrokeError> {
    let device = [
        matrix[0] * direction[0] + matrix[2] * direction[1],
        matrix[1] * direction[0] + matrix[3] * direction[1],
    ];
    let dominant = device[0].abs().max(device[1].abs());
    if !dominant.is_finite() || dominant == 0. {
        return Err(EpsStrokeError::Range);
    }
    let end = [
        point[0] + direction[0] / dominant,
        point[1] + direction[1] / dominant,
    ];
    if end.iter().all(|x| x.is_finite()) && end != point {
        Ok(end)
    } else {
        Err(EpsStrokeError::Range)
    }
}
fn subdivide_dash_polyline<F: FnMut() -> bool>(
    points: &[EpsPoint],
    closed: bool,
    pattern: EpsDashPattern,
    max_segments: usize,
    max_work: usize,
    budget: &MemoryBudget,
    mut cancelled: F,
    visible_zero_metric: Option<crate::EpsMatrix>,
) -> Result<SharedBuffer<S>, EpsStrokeError> {
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
    let run = |emit: &mut dyn FnMut(S) -> Result<(), EpsStrokeError>| -> Result<usize, EpsStrokeError> {
        tick()?;
        for p in points {
            tick()?;
            if !p.iter().all(|x| x.is_finite()) {
                return Err(EpsStrokeError::Range);
            }
        }
        let mut count = 0usize;
        let mut append = |s| {
            tick()?;
            count = count.checked_add(1).ok_or(EpsStrokeError::Limit)?;
            if count > max_segments {
                return Err(EpsStrokeError::Limit);
            }
            emit(s)
        };
        let Some(&origin) = points.first() else {
            return Ok(0);
        };
        if pattern.is_solid() {
            append(S::Move(origin))?;
            for pair in points.windows(2) {
                append(S::Line {
                    start: pair[0],
                    end: pair[1],
                })?;
            }
            if closed && points.len() > 1 {
                append(S::Close {
                    start: *points.last().unwrap(),
                    end: origin,
                })?;
            }
            return Ok(count);
        }
        let length = pattern.length;
        let cycle = length * if length % 2 == 0 { 1 } else { 2 };
        let period = pattern.items().iter().sum::<f64>() * if length % 2 == 0 { 1. } else { 2. };
        let mut phase = pattern.offset.rem_euclid(period);
        let mut index = 0usize;
        while phase > 0. {
            tick()?;
            let item = pattern.items[index % length];
            if phase < item {
                break;
            }
            phase -= item;
            index = (index + 1) % cycle;
        }
        let mut remaining = pattern.items[index % length] - phase;
        let mut active = false;
        let mut any_edge = false;
        let mut pending_vertex_fragment = false;
        let edges = points.len().saturating_sub(1) + usize::from(closed && points.len() > 1);
        for at in 0..edges {
            tick()?;
            let a = points[at];
            let b = points[(at + 1) % points.len()];
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let distance = dx.hypot(dy);
            if !distance.is_finite() {
                return Err(EpsStrokeError::Range);
            }
            if distance == 0. {
                continue;
            }
            any_edge = true;
            let mut consumed = 0.;
            let mut position = a;
            loop {
                tick()?;
                if remaining == 0. {
                    // A positive on-run ending exactly at an interior vertex
                    // still has an oriented zero fragment on the next edge.
                    // Delay the phase transition until that direction is known.
                    if visible_zero_metric.is_some()
                        && index % 2 == 0
                        && pattern.items[index % length] > 0.
                        && consumed == distance
                        && at + 1 < edges
                    {
                        pending_vertex_fragment = true;
                        break;
                    }
                    if index % 2 == 0 && (pattern.items[index % length] == 0. || pending_vertex_fragment) {
                        if !pending_vertex_fragment {
                            append(S::Move(position))?;
                        }
                        let end = if let Some(metric) = visible_zero_metric {
                            visible_dash_endpoint(position, [dx, dy], metric)?
                        } else {
                            position
                        };
                        append(S::Line { start: position, end })?;
                    }
                    pending_vertex_fragment = false;
                    active = false;
                    index = (index + 1) % cycle;
                    remaining = pattern.items[index % length];
                    continue;
                }
                if consumed == distance {
                    break;
                }
                let step = remaining.min(distance - consumed);
                let next = consumed + step;
                // Refuse numerical non-progress instead of looping at huge scales.
                if next <= consumed || !next.is_finite() {
                    return Err(EpsStrokeError::Range);
                }
                let endpoint = if next == distance {
                    b
                } else {
                    [a[0] + dx * (next / distance), a[1] + dy * (next / distance)]
                };
                if index % 2 == 0 {
                    if !active {
                        append(S::Move(position))?;
                        active = true;
                    }
                    append(S::Line {
                        start: position,
                        end: endpoint,
                    })?;
                } else {
                    active = false;
                }
                position = endpoint;
                consumed = next;
                remaining -= step;
            }
        }
        if !any_edge && points.len() > 1 && index % 2 == 0 {
            append(S::Move(origin))?;
            append(S::Line {
                start: origin,
                end: origin,
            })?;
        }
        Ok(count)
    };
    let mut first_line = None;
    let mut last_line = None;
    let mut last_move = 0;
    let mut moves = 0usize;
    let mut at = 0usize;
    let count = run(&mut |segment| {
        match segment {
            S::Move(_) => {
                last_move = at;
                moves += 1;
            }
            S::Line { start, end } => {
                first_line.get_or_insert((start, end));
                last_line = Some((start, end));
            }
            _ => {}
        }
        at += 1;
        Ok(())
    })?;
    let seam = closed
        && !pattern.is_solid()
        && first_line.is_some_and(|(a, b)| points.first() == Some(&a) && a != b)
        && last_line.is_some_and(|(a, b)| points.first() == Some(&b) && a != b);
    let merge = seam && moves > 1;
    let size = count - usize::from(merge);
    tick()?;
    let mut output = budget.try_buffer(size, S::Move([0.; 2]))?;
    let mut at = 0usize;
    run(&mut |mut segment| {
        let source = at;
        at += 1;
        if seam && moves == 1 && source + 1 == count {
            if let S::Line { start, end } = segment {
                segment = S::Close { start, end };
            }
        }
        if merge && source == 0 {
            return Ok(());
        }
        let destination = if !merge {
            source
        } else if source >= last_move {
            source - last_move
        } else {
            count - last_move + source - 1
        };
        output[destination] = segment;
        Ok(())
    })?;
    tick()?;
    Ok(output.freeze())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn split(points: &[EpsPoint], closed: bool, items: &[f64], phase: f64) -> Vec<S> {
        let root = MemoryBudget::new(100_000);
        let result = dash_eps_polyline(
            points,
            closed,
            EpsDashPattern::new(items, phase).unwrap(),
            1000,
            10000,
            &root,
            || false,
        )
        .unwrap();
        let values = result.to_vec();
        drop(result);
        assert_eq!(root.used(), 0);
        values
    }
    fn line(a: EpsPoint, b: EpsPoint) -> S {
        S::Line { start: a, end: b }
    }
    #[test]
    fn dash_all_pixels_independent_closed_corner_qualification() {
        compare_dash_reference_pixels();
    }
    fn compare_dash_reference_pixels() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/dash-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["pixel_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 16);
        for case in cases {
            let root = MemoryBudget::new(32 * 1024 * 1024);
            let source = format!(
                "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n1 setgray 0 0 8 8 rectfill 0 setgray\n{}\n%%EOF\n",
                case["source"].as_str().unwrap()
            );
            let raster = crate::decode_eps_document(
                source.as_bytes(),
                1.,
                crate::EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                crate::EpsDocumentLimits::default(),
                &root,
                || false,
            )
            .unwrap();
            let rrrah_core::RasterPixels::Rgba8(pixels) = raster.pixels() else {
                panic!()
            };
            let expected: Vec<u8> = case["rgb"]
                .as_array()
                .unwrap()
                .chunks_exact(3)
                .flat_map(|p| {
                    [
                        p[0].as_u64().unwrap() as u8,
                        p[1].as_u64().unwrap() as u8,
                        p[2].as_u64().unwrap() as u8,
                        255,
                    ]
                })
                .collect();
            assert_eq!(&pixels[..], &expected[..], "{}", case["name"]);
            drop(raster);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn dash_outline_cancel_work_and_exact_quota_release_all_temporaries() {
        let points = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
        let style = crate::EpsStrokeStyle {
            width: 2.,
            dash: EpsDashPattern::new(&[6., 2.], 2.).unwrap(),
            ..Default::default()
        };
        let root = MemoryBudget::new(1_000_000);
        let mut polls = 0;
        let run = |budget: &MemoryBudget, work, callback: &mut dyn FnMut() -> bool| {
            crate::outline_eps_polyline(
                &points,
                true,
                style,
                [1., 0., 0., 1., 0., 0.],
                0.125,
                10000,
                work,
                budget,
                callback,
            )
        };
        drop(
            run(&root, 100000, &mut || {
                polls += 1;
                false
            })
            .unwrap(),
        );
        assert_eq!(root.used(), 0);
        for stop in 1..=polls {
            let mut at = 0;
            assert!(
                matches!(
                    run(&root, 100000, &mut || {
                        at += 1;
                        at == stop
                    }),
                    Err(EpsStrokeError::Cancelled)
                ),
                "cancel {stop}/{polls}"
            );
            assert_eq!(root.used(), 0);
        }
        let mut found = false;
        for allowance in 0..10000 {
            match run(&root, allowance, &mut || false) {
                Err(EpsStrokeError::Limit) => {}
                Ok(result) => {
                    drop(result);
                    found = true;
                    break;
                }
                other => panic!("work {allowance}: {other:?}"),
            }
            assert_eq!(root.used(), 0);
        }
        assert!(found);
        let (mut low, mut high) = (0, 100000);
        while low < high {
            let mid = (low + high) / 2;
            match run(&root.child(mid), 100000, &mut || false) {
                Ok(result) => {
                    drop(result);
                    high = mid;
                }
                Err(EpsStrokeError::Memory(_)) => low = mid + 1,
                other => panic!("quota {mid}: {other:?}"),
            }
            assert_eq!(root.used(), 0);
        }
        drop(run(&root.child(low), 100000, &mut || false).unwrap());
        assert!(matches!(
            run(&root.child(low - 1), 100000, &mut || false),
            Err(EpsStrokeError::Memory(_))
        ));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn dash_visible_zero_fragments_use_one_device_pixel_and_next_tangent() {
        let root = MemoryBudget::new(100_000);
        let points = [[2., 2.], [6., 2.], [8., 4.]];
        let pattern = EpsDashPattern::new(&[4., 100.], 0.).unwrap();
        let result = dash_eps_polyline_for_stroke(
            &points,
            false,
            pattern,
            [1., 0., 0., 1., 0., 0.],
            0,
            100,
            1000,
            &root,
            || false,
        )
        .unwrap();
        assert_eq!(
            &result[..],
            &[
                S::Move([2., 2.]),
                line([2., 2.], [6., 2.]),
                line([6., 2.], [7., 3.])
            ]
        );
        drop(result);
        let result = dash_eps_polyline_for_stroke(
            &points,
            false,
            pattern,
            [2., 0., 0., 3., 0., 0.],
            0,
            100,
            1000,
            &root,
            || false,
        )
        .unwrap();
        let S::Line { start, end } = result[2] else {
            panic!()
        };
        let delta = [2. * (end[0] - start[0]), 3. * (end[1] - start[1])];
        assert!((delta[0].abs().max(delta[1].abs()) - 1.).abs() < 1e-12);
        assert!((delta[0] - 2. / 3.).abs() < 1e-12);
        drop(result);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn dash_curve_endpoint_uses_user_arc_length_under_nonuniform_transform() {
        let root = MemoryBudget::new(1_000_000);
        let mut g = crate::EpsGraphics::new(
            crate::EpsGraphicsLimits {
                max_nodes: 32,
                max_paints: 1,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.set_dash(EpsDashPattern::new(&[1., 1.], 0.).unwrap());
        g.scale(2., 1.).unwrap();
        g.arc([0., 0., 3., 0., 90.], false, || false).unwrap();
        g.paint(crate::EpsPaintKind::Stroke).unwrap();
        let scene = g.finish();
        let mut limits = crate::EpsFlattenLimits::default();
        limits.tolerance = 0.001;
        let prepared =
            crate::prepare_eps_stroke(&scene, 0, [1., 0., 0., 1., 0., 0.], limits, &root, || false).unwrap();
        let mut points = Vec::new();
        for segment in prepared.path.iter() {
            match *segment {
                S::Move(p) => points.push(p),
                S::Line { end, .. } => points.push(end),
                _ => panic!("unflattened arc"),
            }
        }
        let dash = dash_eps_polyline(&points, false, prepared.style.dash, 10000, 100000, &root, || {
            false
        })
        .unwrap();
        let first_end = dash
            .iter()
            .skip(1)
            .take_while(|s| !matches!(s, S::Move(_)))
            .last()
            .unwrap();
        let S::Line { end, .. } = *first_end else { panic!() };
        let expected = [3. * (1f64 / 3.).cos(), 3. * (1f64 / 3.).sin()];
        assert!(
            (end[0] - expected[0]).hypot(end[1] - expected[1]) < 0.002,
            "{end:?} versus analytic circle {expected:?}"
        );
        drop((dash, prepared, scene));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn dash_hairline_metric_is_user_space_and_pen_is_device_space() {
        let root = MemoryBudget::new(1_000_000);
        let mut g = crate::EpsGraphics::new(
            crate::EpsGraphicsLimits {
                max_nodes: 16,
                max_paints: 1,
                max_saved_states: 0,
            },
            &root,
            || false,
        )
        .unwrap();
        g.set_dash(EpsDashPattern::new(&[1., 1.], 0.).unwrap());
        g.set_line_width(0.).unwrap();
        g.scale(2., 3.).unwrap();
        g.move_to(0., 1.).unwrap();
        g.line_to(4., 1.).unwrap();
        g.paint(crate::EpsPaintKind::Stroke).unwrap();
        let scene = g.finish();
        let prepared = crate::prepare_eps_stroke(
            &scene,
            0,
            [1., 0., 0., 1., 0., 0.],
            crate::EpsFlattenLimits::default(),
            &root,
            || false,
        )
        .unwrap();
        assert_eq!(prepared.style.width, 1.);
        let outline = crate::outline_eps_prepared_stroke(
            &prepared,
            crate::EpsStrokeAssemblyLimits::default(),
            &root,
            || false,
        )
        .unwrap();
        let mask = crate::fill_eps_path(
            &outline,
            8,
            6,
            false,
            crate::EpsFillLimits::default(),
            &root,
            || false,
        )
        .unwrap();
        for (i, value) in mask.iter().enumerate() {
            let (x, y) = (i % 8, i / 8);
            assert_eq!(
                *value,
                if [0, 1, 4, 5].contains(&x) && [2, 3].contains(&y) {
                    128
                } else {
                    0
                },
                "{x},{y}"
            );
        }
        drop((mask, outline, prepared, scene));
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn dash_phase_odd_zero_corner_and_closed_seam_geometry() {
        let points = [[0., 0.], [10., 0.]];
        assert_eq!(
            split(&points, false, &[2.], 0.),
            split(&points, false, &[2., 2.], 0.)
        );
        assert_eq!(
            split(&points, false, &[2.], 1.),
            vec![
                S::Move([0., 0.]),
                line([0., 0.], [1., 0.]),
                S::Move([3., 0.]),
                line([3., 0.], [5., 0.]),
                S::Move([7., 0.]),
                line([7., 0.], [9., 0.])
            ]
        );
        assert_eq!(
            split(&points, false, &[2.], -1.),
            vec![
                S::Move([1., 0.]),
                line([1., 0.], [3., 0.]),
                S::Move([5., 0.]),
                line([5., 0.], [7., 0.]),
                S::Move([9., 0.]),
                line([9., 0.], [10., 0.])
            ]
        );
        assert_eq!(
            split(&[[0., 0.], [4., 0.]], false, &[0., 2.], 0.),
            vec![
                S::Move([0., 0.]),
                line([0., 0.], [0., 0.]),
                S::Move([2., 0.]),
                line([2., 0.], [2., 0.]),
                S::Move([4., 0.]),
                line([4., 0.], [4., 0.])
            ]
        );
        assert_eq!(
            split(&[[0., 0.], [2., 0.], [2., 2.]], false, &[3., 2.], 0.),
            vec![
                S::Move([0., 0.]),
                line([0., 0.], [2., 0.]),
                line([2., 0.], [2., 1.])
            ]
        );
        let square = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
        assert_eq!(
            split(&square, true, &[6., 2.], 2.),
            vec![
                S::Move([0., 2.]),
                line([0., 2.], [0., 0.]),
                line([0., 0.], [4., 0.]),
                S::Move([4., 2.]),
                line([4., 2.], [4., 4.]),
                line([4., 4.], [0., 4.])
            ]
        );
        let all = split(&square, true, &[100., 1.], 0.);
        assert!(matches!(
            all.last(),
            Some(S::Close {
                start: [0., 4.],
                end: [0., 0.]
            })
        ));
    }
    #[test]
    fn dash_count_write_cancel_work_and_exact_quota_are_bounded() {
        let points = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
        let pattern = EpsDashPattern::new(&[6., 2.], 2.).unwrap();
        let root = MemoryBudget::new(100_000);
        let mut polls = 0;
        let result = dash_eps_polyline(&points, true, pattern, 1000, 10000, &root, || {
            polls += 1;
            false
        })
        .unwrap();
        let bytes = result.capacity_bytes();
        let clone = result.clone();
        assert!(clone.ptr_eq(&result));
        drop(result);
        assert_eq!(root.used(), bytes);
        drop(clone);
        assert_eq!(root.used(), 0);
        for stop in 1..=polls {
            let mut at = 0;
            assert!(
                matches!(
                    dash_eps_polyline(&points, true, pattern, 1000, 10000, &root, || {
                        at += 1;
                        at == stop
                    }),
                    Err(EpsStrokeError::Cancelled)
                ),
                "point {stop}"
            );
            assert_eq!(root.used(), 0);
        }
        drop(dash_eps_polyline(&points, true, pattern, 1000, 10000, &root.child(bytes), || false).unwrap());
        assert!(matches!(
            dash_eps_polyline(
                &points,
                true,
                pattern,
                1000,
                10000,
                &root.child(bytes - 1),
                || false
            ),
            Err(EpsStrokeError::Memory(_))
        ));
        for work in 0..polls {
            assert!(matches!(
                dash_eps_polyline(&points, true, pattern, 1000, work, &root, || false),
                Err(EpsStrokeError::Limit)
            ));
            assert_eq!(root.used(), 0);
        }
        assert!(matches!(
            dash_eps_polyline(
                &[[0., 0.], [1e300, 0.]],
                false,
                EpsDashPattern::new(&[1e-300], 0.).unwrap(),
                1000,
                32,
                &root,
                || false
            ),
            Err(EpsStrokeError::Limit)
        ));
        for items in [&[0., 0.][..], &[-1., 2.], &[f64::NAN, 1.]] {
            assert!(matches!(
                EpsDashPattern::new(items, 0.),
                Err(EpsStrokeError::Range)
            ));
        }
        assert!(matches!(
            EpsDashPattern::new(&[1.; EPS_MAX_DASH_ITEMS + 1], 0.),
            Err(EpsStrokeError::Limit)
        ));
        assert_eq!(root.used(), 0);
    }
}
