//! Test-only admitted polygon stroke expansion. Curved dash phase and automatic
//! stroke adjustment still require qualification before production routing.
use super::bounded_stroke::{Run, StrokeRuns};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Stroke, Vec2};
pub(super) struct StrokeOutline {
    pub path: BezPath,
    pub(super) _credit: Box<dyn std::any::Any + Send + Sync>,
}
struct Writer<'a> {
    count: usize,
    cap: usize,
    work: usize,
    transform: Affine,
    cancelled: &'a dyn Fn() -> bool,
    output: Option<Vec<PathEl>>,
}
impl Writer<'_> {
    fn step(&mut self) -> Option<()> {
        if (self.cancelled)() {
            return None;
        }
        self.work = self.work.checked_sub(1)?;
        Some(())
    }
    fn push(&mut self, element: PathEl) -> Option<()> {
        self.step()?;
        if self.count >= self.cap {
            return None;
        }
        let element = match element {
            PathEl::MoveTo(p) => PathEl::MoveTo(self.point(p)?),
            PathEl::LineTo(p) => PathEl::LineTo(self.point(p)?),
            PathEl::ClosePath => PathEl::ClosePath,
            _ => return None,
        };
        self.count += 1;
        if let Some(out) = &mut self.output {
            out.push(element);
        }
        Some(())
    }
    fn point(&self, p: Point) -> Option<Point> {
        let q = self.transform * p;
        if q.x.is_finite() && q.y.is_finite() && q.x.abs() <= 1_048_576. && q.y.abs() <= 1_048_576. {
            Some(q)
        } else {
            None
        }
    }
    fn polygon(&mut self, points: &[Point]) -> Option<()> {
        self.step()?;
        let mut twice = 0.;
        for pair in points[1..].windows(2) {
            twice += (pair[0] - points[0]).cross(pair[1] - points[0]);
        }
        if !twice.is_finite() {
            return None;
        }
        if twice == 0. {
            return Some(());
        }
        self.push(PathEl::MoveTo(points[0]))?;
        if twice > 0. {
            for &p in &points[1..] {
                self.push(PathEl::LineTo(p))?;
            }
        } else {
            for &p in points[1..].iter().rev() {
                self.push(PathEl::LineTo(p))?;
            }
        }
        self.push(PathEl::ClosePath)
    }
    fn disk(&mut self, p: Point, r: f64, tolerance: f64) -> Option<()> {
        let count = arc_count(std::f64::consts::TAU, r, tolerance)?.max(4);
        if count.checked_add(1)? > self.cap - self.count {
            return None;
        }
        for i in 0..count {
            let angle = std::f64::consts::TAU * i as f64 / count as f64;
            let point = p + Vec2::new(angle.cos() * r, angle.sin() * r);
            self.push(if i == 0 {
                PathEl::MoveTo(point)
            } else {
                PathEl::LineTo(point)
            })?;
        }
        self.push(PathEl::ClosePath)
    }
    fn sector(
        &mut self,
        p: Point,
        r: f64,
        first: Vec2,
        last: Vec2,
        sweep: f64,
        tolerance: f64,
    ) -> Option<()> {
        if sweep == 0. {
            return Some(());
        }
        let (first, last) = if sweep < 0. { (last, first) } else { (first, last) };
        let sweep = sweep.abs();
        let count = arc_count(sweep, r, tolerance)?.max(1);
        if count.checked_add(3)? > self.cap - self.count {
            return None;
        }
        self.push(PathEl::MoveTo(p))?;
        self.push(PathEl::LineTo(p + first * r))?;
        let start = first.y.atan2(first.x);
        for i in 1..count {
            let angle = start + sweep * i as f64 / count as f64;
            self.push(PathEl::LineTo(p + Vec2::new(angle.cos() * r, angle.sin() * r)))?;
        }
        self.push(PathEl::LineTo(p + last * r))?;
        self.push(PathEl::ClosePath)
    }
}
fn arc_count(angle: f64, radius: f64, tolerance: f64) -> Option<usize> {
    // 1-cos(theta/2) <= theta²/8, so this chord step bounds sagitta.
    let step = (8. * tolerance / radius).sqrt();
    if !step.is_finite() || step <= 0. {
        return None;
    }
    let count = (angle / step).ceil();
    if !count.is_finite() || count > 1_000_000. {
        None
    } else {
        Some(count as usize)
    }
}
fn direction(a: Point, b: Point) -> Option<Vec2> {
    let vector = b - a;
    let length = vector.hypot();
    if length > 0. && length.is_finite() {
        Some(vector / length)
    } else {
        None
    }
}
fn normal(u: Vec2) -> Vec2 {
    Vec2::new(-u.y, u.x)
}
fn cap(w: &mut Writer<'_>, p: Point, u: Vec2, r: f64, style: Cap, start: bool, tol: f64) -> Option<()> {
    let n = normal(u);
    match style {
        Cap::Butt => Some(()),
        Cap::Square => {
            let extension = u * (if start { -r } else { r });
            w.polygon(&[p - n * r, p + n * r, p + n * r + extension, p - n * r + extension])
        }
        Cap::Round => {
            let (a, b) = if start { (n, -n) } else { (-n, n) };
            w.sector(p, r, a, b, std::f64::consts::PI, tol)
        }
    }
}
fn join(
    w: &mut Writer<'_>,
    p: Point,
    before: Vec2,
    after: Vec2,
    r: f64,
    style: Join,
    limit: f64,
    tol: f64,
) -> Option<()> {
    w.step()?;
    let cross = before.cross(after);
    let dot = before.dot(after);
    if cross == 0. {
        if dot < 0. && style == Join::Round {
            return w.disk(p, r, tol);
        }
        return Some(());
    }
    let sign = if cross > 0. { -1. } else { 1. };
    let na = normal(before) * sign;
    let nb = normal(after) * sign;
    let a = p + na * r;
    let b = p + nb * r;
    match style {
        Join::Round => w.sector(p, r, na, nb, cross.atan2(dot), tol),
        Join::Bevel => w.polygon(&[p, a, b]),
        Join::Miter => {
            let at = a + before * ((b - a).cross(after) / cross);
            if !at.x.is_finite() || !at.y.is_finite() {
                return None;
            }
            if (at - p).hypot() / r <= limit {
                w.polygon(&[p, a, at, b])
            } else {
                w.polygon(&[p, a, b])
            }
        }
    }
}
fn run_points<'a>(plan: &'a StrokeRuns, run: Run) -> Option<&'a [Point]> {
    plan.points.get(run.start..run.start.checked_add(run.len)?)
}
fn mapped(p: Point, hairline: bool, transform: Affine) -> Point {
    if hairline { transform * p } else { p }
}
fn first_direction(points: &[Point], hairline: bool, transform: Affine) -> Option<Vec2> {
    points.windows(2).find_map(|p| {
        direction(
            mapped(p[0], hairline, transform),
            mapped(p[1], hairline, transform),
        )
    })
}
fn last_direction(points: &[Point], hairline: bool, transform: Affine) -> Option<Vec2> {
    points.windows(2).rev().find_map(|p| {
        direction(
            mapped(p[0], hairline, transform),
            mapped(p[1], hairline, transform),
        )
    })
}
fn walk(
    plan: &StrokeRuns,
    stroke: &Stroke,
    transform: Affine,
    r: f64,
    tol: f64,
    w: &mut Writer<'_>,
) -> Option<()> {
    let hairline = stroke.width == 0.;
    for subpath in &plan.subpaths {
        w.step()?;
        let runs = plan
            .runs
            .get(subpath.first_run..subpath.first_run.checked_add(subpath.run_count)?)?;
        if runs.is_empty() {
            // PDF single-point open subpaths paint nothing. Degenerate closed
            // subpaths paint only with round caps; dashed degeneracy remains pending.
            if subpath.closed && stroke.dash_pattern.is_empty() && stroke.start_cap == Cap::Round {
                w.disk(mapped(subpath.start, hairline, transform), r, tol)?;
            }
            continue;
        }
        let first = run_points(plan, runs[0])?;
        let last = run_points(plan, *runs.last()?)?;
        let seam = subpath.closed
            && first.len() > 1
            && last.len() > 1
            && first.first() == Some(&subpath.start)
            && last.last() == Some(&subpath.start);
        let seam_before = last_direction(last, hairline, transform);
        let seam_after = first_direction(first, hairline, transform);
        let seam = seam && seam_before.is_some() && seam_after.is_some();
        for (index, &run) in runs.iter().enumerate() {
            w.step()?;
            let points = run_points(plan, run)?;
            let p = *points.first()?;
            let first_u = first_direction(points, hairline, transform);
            let last_u = last_direction(points, hairline, transform);
            if first_u.is_none() {
                let p = mapped(p, hairline, transform);
                let tangent = run.dot_tangent.and_then(|t| {
                    if hairline {
                        direction(p, transform * (points[0] + t))
                    } else {
                        Some(t)
                    }
                });
                if let Some(u) = tangent {
                    cap(w, p, u, r, stroke.start_cap, true, tol)?;
                    cap(w, p, u, r, stroke.end_cap, false, tol)?;
                } else if stroke.start_cap == Cap::Round || stroke.end_cap == Cap::Round {
                    w.disk(p, r, tol)?;
                }
                continue;
            }
            let mut previous = None;
            for pair in points.windows(2) {
                w.step()?;
                let (a, b) = (
                    mapped(pair[0], hairline, transform),
                    mapped(pair[1], hairline, transform),
                );
                let Some(u) = direction(a, b) else {
                    continue;
                };
                let n = normal(u) * r;
                w.polygon(&[a - n, b - n, b + n, a + n])?;
                if let Some(before) = previous {
                    join(w, a, before, u, r, stroke.join, stroke.miter_limit, tol)?;
                }
                previous = Some(u);
            }
            if !(seam && index == 0) {
                cap(
                    w,
                    mapped(p, hairline, transform),
                    first_u?,
                    r,
                    stroke.start_cap,
                    true,
                    tol,
                )?;
            }
            if !(seam && index == runs.len() - 1) {
                cap(
                    w,
                    mapped(*points.last()?, hairline, transform),
                    last_u?,
                    r,
                    stroke.end_cap,
                    false,
                    tol,
                )?;
            }
        }
        if seam {
            join(
                w,
                mapped(subpath.start, hairline, transform),
                seam_before?,
                seam_after?,
                r,
                stroke.join,
                stroke.miter_limit,
                tol,
            )?;
        }
    }
    Some(())
}

/// Union positively wound strip/join/cap polygons without allocating geometry
/// before exact-size admission. Round arcs have a device-space sagitta bound.
pub(super) fn admitted_stroke_outline(
    plan: &StrokeRuns,
    stroke: &Stroke,
    transform: Affine,
    device_tolerance: f64,
    max_elements: usize,
    work_limit: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<StrokeOutline> {
    if cancelled()
        || !stroke.width.is_finite()
        || stroke.width < 0.
        || !stroke.miter_limit.is_finite()
        || stroke.miter_limit < 1.
        || !device_tolerance.is_finite()
        || device_tolerance <= 0.
        || max_elements == 0
        || max_elements > 1_000_000
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
    {
        return None;
    }
    let c = transform.as_coeffs();
    let norm = c[0].hypot(c[1]).hypot(c[2].hypot(c[3]));
    if !norm.is_finite() || norm <= 0. {
        return None;
    }
    let hairline = stroke.width == 0.;
    let r = if hairline { 0.5 } else { stroke.width * 0.5 };
    if r == 0. {
        return None;
    }
    let tolerance = if hairline {
        device_tolerance
    } else {
        device_tolerance / norm
    };
    let output_transform = if hairline { Affine::IDENTITY } else { transform };
    let mut writer = Writer {
        count: 0,
        cap: max_elements,
        work: work_limit,
        transform: output_transform,
        cancelled,
        output: None,
    };
    walk(plan, stroke, transform, r, tolerance, &mut writer)?;
    let count = writer.count;
    let work = writer.work;
    let credit = admit(count.checked_mul(size_of::<PathEl>())?)?;
    if cancelled() {
        return None;
    }
    let mut output = Vec::new();
    output.try_reserve_exact(count).ok()?;
    let mut writer = Writer {
        count: 0,
        cap: count,
        work,
        transform: output_transform,
        cancelled,
        output: Some(output),
    };
    walk(plan, stroke, transform, r, tolerance, &mut writer)?;
    if writer.count != count {
        return None;
    }
    Some(StrokeOutline {
        path: BezPath::from_vec(writer.output?),
        _credit: credit,
    })
}

#[cfg(test)]
fn outline(path: &BezPath, stroke: &Stroke, transform: Affine) -> StrokeOutline {
    let plan = super::bounded_stroke::admitted_stroke_runs(
        path,
        stroke,
        transform,
        0.0001,
        100_000,
        100_000,
        20_000_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    admitted_stroke_outline(
        &plan,
        stroke,
        transform,
        0.0001,
        1_000_000,
        20_000_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap()
}
#[cfg(test)]
fn area(path: &BezPath, x: i32, y: i32) -> f64 {
    super::polygon_coverage::polygon_pixel_area(
        &[(path, vello_cpu::peniko::Fill::NonZero)],
        x,
        y,
        20_000_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap()
}
#[test]
fn independently_authored_cap_and_corner_areas() {
    let mut line = BezPath::new();
    line.move_to((0., 1.));
    line.line_to((2., 1.));
    for (cap_style, expected) in [(Cap::Butt, 0), (Cap::Square, 255), (Cap::Round, 200)] {
        let stroke = Stroke {
            width: 2.,
            start_cap: cap_style,
            end_cap: cap_style,
            ..Stroke::default()
        };
        let output = outline(&line, &stroke, Affine::IDENTITY);
        assert_eq!((area(&output.path, -1, 0) * 255.).round() as u8, expected);
        assert_eq!(area(&output.path, 0, 0), 1.);
    }
    let mut corner = BezPath::new();
    corner.move_to((-2., 0.));
    corner.line_to((0., 0.));
    corner.line_to((0., 2.));
    for (join_style, limit, expected) in [
        (Join::Miter, 4., 255),
        (Join::Miter, 1., 128),
        (Join::Bevel, 4., 128),
        (Join::Round, 4., 200),
    ] {
        let stroke = Stroke {
            width: 2.,
            join: join_style,
            miter_limit: limit,
            ..Stroke::default()
        };
        let output = outline(&corner, &stroke, Affine::IDENTITY);
        assert_eq!(
            (area(&output.path, 0, -1) * 255.).round() as u8,
            expected,
            "join={join_style:?}"
        );
        let reflected = outline(&corner, &stroke, Affine::scale_non_uniform(-1., 1.));
        assert_eq!((area(&reflected.path, -1, -1) * 255.).round() as u8, expected);
    }
}
#[test]
fn closed_seam_joins_and_device_hairline_preserve_independent_pixels() {
    let mut path = BezPath::new();
    path.move_to((1., 1.));
    path.line_to((5., 1.));
    path.line_to((5., 5.));
    path.line_to((1., 5.));
    path.line_to((1., 1.));
    let stroke = Stroke {
        width: 2.,
        join: Join::Miter,
        start_cap: Cap::Butt,
        end_cap: Cap::Butt,
        ..Stroke::default()
    };
    assert_eq!(area(&outline(&path, &stroke, Affine::IDENTITY).path, 0, 0), 0.);
    path.close_path();
    assert_eq!(area(&outline(&path, &stroke, Affine::IDENTITY).path, 0, 0), 1.);
    let mut line = BezPath::new();
    line.move_to((0., 0.));
    line.line_to((2., 0.));
    for scale in [1., 10.] {
        let stroke = Stroke {
            width: 0.,
            ..Stroke::default()
        };
        assert_eq!(
            area(&outline(&line, &stroke, Affine::scale(scale)).path, 1, 0),
            0.5
        );
    }
}
#[test]
fn zero_length_dash_square_orientation_differs_from_degenerate_subpath() {
    let mut line = BezPath::new();
    line.move_to((0., 1.));
    line.line_to((8., 1.));
    let stroke = Stroke {
        width: 2.,
        start_cap: Cap::Square,
        end_cap: Cap::Square,
        dash_pattern: [0., 4.].into_iter().collect(),
        ..Stroke::default()
    };
    let output = outline(&line, &stroke, Affine::IDENTITY);
    assert_eq!(area(&output.path, -1, 0), 1.);
    assert_eq!(area(&output.path, 1, 0), 0.);
    let mut point = BezPath::new();
    point.move_to((0., 0.));
    point.line_to((0., 0.));
    for (cap_style, expected) in [(Cap::Butt, 0), (Cap::Square, 0), (Cap::Round, 200)] {
        let stroke = Stroke {
            width: 2.,
            start_cap: cap_style,
            end_cap: cap_style,
            ..Stroke::default()
        };
        assert_eq!(
            (area(&outline(&point, &stroke, Affine::IDENTITY).path, 0, 0) * 255.).round() as u8,
            expected
        );
    }
}

#[test]
fn outline_refusal_cancellation_and_vertex_limit_release_all_output_credit() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Credit(Arc<AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::Relaxed);
        }
    }
    let mut path = BezPath::new();
    path.move_to((0., 0.));
    path.line_to((2., 0.));
    path.line_to((2., 2.));
    let stroke = Stroke {
        width: 2.,
        start_cap: Cap::Round,
        end_cap: Cap::Round,
        join: Join::Round,
        ..Stroke::default()
    };
    let plan = super::bounded_stroke::admitted_stroke_runs(
        &path,
        &stroke,
        Affine::IDENTITY,
        0.0001,
        10_000,
        10_000,
        1_000_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    let checks = Cell::new(0);
    let used = Arc::new(AtomicUsize::new(0));
    let output = admitted_stroke_outline(
        &plan,
        &stroke,
        Affine::IDENTITY,
        0.0001,
        10_000,
        1_000_000,
        &|| {
            checks.set(checks.get() + 1);
            false
        },
        &|bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)))
        },
    )
    .unwrap();
    assert_eq!(
        used.load(Ordering::Relaxed),
        output.path.elements().len() * size_of::<PathEl>()
    );
    drop(output);
    assert_eq!(used.load(Ordering::Relaxed), 0);
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        assert!(
            admitted_stroke_outline(
                &plan,
                &stroke,
                Affine::IDENTITY,
                0.0001,
                10_000,
                1_000_000,
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                },
                &|bytes| {
                    used.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone(), bytes)))
                }
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0, "cancel={stop}");
    }
    assert!(
        admitted_stroke_outline(
            &plan,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            10_000,
            1_000_000,
            &|| false,
            &|_| None
        )
        .is_none()
    );
    assert!(
        admitted_stroke_outline(
            &plan,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            8,
            1_000_000,
            &|| false,
            &|_| panic!("vertex-exhausted outline admitted")
        )
        .is_none()
    );
    assert!(
        admitted_stroke_outline(
            &plan,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            10_000,
            1,
            &|| false,
            &|_| panic!("work-exhausted outline admitted")
        )
        .is_none()
    );
}
