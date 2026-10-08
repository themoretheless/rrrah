//! Test-only admitted stroke-run preparation. Outline geometry is a separate
//! stage; this does not yet replace the renderer's stroker.
use kurbo::{Affine, BezPath, PathEl, Point, Stroke, Vec2};

#[derive(Clone, Copy, Debug)]
pub(super) struct Run {
    pub start: usize,
    pub len: usize,
    pub dot_tangent: Option<Vec2>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Subpath {
    pub first_run: usize,
    pub run_count: usize,
    pub closed: bool,
    pub start: Point,
}
pub(super) struct StrokeRuns {
    pub points: Vec<Point>,
    pub runs: Vec<Run>,
    pub subpaths: Vec<Subpath>,
    // All geometry drops before its admitted storage credit.
    _credit: Box<dyn std::any::Any + Send + Sync>,
}
struct Sink<'a> {
    points: usize,
    runs: usize,
    subpaths: usize,
    pending: Option<(usize, Option<Vec2>)>,
    first_run: usize,
    subpath_start: Point,
    max_points: usize,
    max_runs: usize,
    output: Option<(&'a mut Vec<Point>, &'a mut Vec<Run>, &'a mut Vec<Subpath>)>,
}
impl Sink<'_> {
    fn begin_subpath(&mut self, start: Point) -> Option<()> {
        self.first_run = self.runs;
        self.subpath_start = start;
        Some(())
    }
    fn begin_run(&mut self, p: Point, tangent: Option<Vec2>) -> Option<()> {
        if self.pending.is_some() {
            return None;
        }
        self.pending = Some((self.points, tangent));
        self.point(p)
    }
    fn point(&mut self, p: Point) -> Option<()> {
        if !p.x.is_finite() || !p.y.is_finite() || self.points >= self.max_points {
            return None;
        }
        self.points += 1;
        if let Some((points, _, _)) = &mut self.output {
            points.push(p);
        }
        Some(())
    }
    fn end_run(&mut self) -> Option<()> {
        if let Some((start, dot_tangent)) = self.pending.take() {
            if self.runs >= self.max_runs {
                return None;
            }
            self.runs += 1;
            if let Some((_, runs, _)) = &mut self.output {
                runs.push(Run {
                    start,
                    len: self.points - start,
                    dot_tangent,
                });
            }
        }
        Some(())
    }
    fn end_subpath(&mut self, closed: bool) -> Option<()> {
        self.end_run()?;
        if self.subpaths >= self.max_runs {
            return None;
        }
        self.subpaths += 1;
        if let Some((_, _, subpaths)) = &mut self.output {
            subpaths.push(Subpath {
                first_run: self.first_run,
                run_count: self.runs - self.first_run,
                closed,
                start: self.subpath_start,
            });
        }
        Some(())
    }
}
struct Work<'a> {
    left: usize,
    cancelled: &'a dyn Fn() -> bool,
}
impl Work<'_> {
    fn step(&mut self) -> Option<()> {
        if (self.cancelled)() {
            return None;
        }
        self.left = self.left.checked_sub(1)?;
        Some(())
    }
}
#[derive(Clone, Copy)]
struct Dash<'a> {
    pattern: &'a [f64],
    index: usize,
    remaining: f64,
}
impl<'a> Dash<'a> {
    fn reset(pattern: &'a [f64], offset: f64) -> Option<Self> {
        if pattern.is_empty() {
            return Some(Self {
                pattern,
                index: 0,
                remaining: f64::INFINITY,
            });
        }
        let count = if pattern.len() % 2 == 0 {
            pattern.len()
        } else {
            pattern.len().checked_mul(2)?
        };
        let period = pattern.iter().try_fold(0., |a, b| {
            let sum = a + b;
            if sum.is_finite() { Some(sum) } else { None }
        })? * if pattern.len() % 2 == 0 { 1. } else { 2. };
        if !period.is_finite() || period <= 0. {
            return None;
        }
        let mut phase = offset.rem_euclid(period);
        let mut index = 0;
        while index < count {
            let length = pattern[index % pattern.len()];
            if phase < length || (phase == 0. && length == 0.) {
                break;
            }
            phase -= length;
            index += 1;
        }
        if index == count {
            return None;
        }
        Some(Self {
            pattern,
            index,
            remaining: pattern[index % pattern.len()] - phase,
        })
    }
    fn on(&self) -> bool {
        self.pattern.is_empty() || self.index % 2 == 0
    }
    fn advance(&mut self) -> Option<()> {
        let count = if self.pattern.len() % 2 == 0 {
            self.pattern.len()
        } else {
            self.pattern.len().checked_mul(2)?
        };
        self.index = (self.index + 1) % count;
        self.remaining = self.pattern[self.index % self.pattern.len()];
        Some(())
    }
    fn zeros(
        &mut self,
        p: Point,
        tangent: Option<Vec2>,
        sink: &mut Sink<'_>,
        work: &mut Work<'_>,
    ) -> Option<()> {
        while !self.pattern.is_empty() && self.remaining == 0. {
            work.step()?;
            if self.on() && self.pattern[self.index % self.pattern.len()] == 0. {
                sink.end_run()?;
                sink.begin_run(p, tangent)?;
                sink.end_run()?;
            }
            self.advance()?;
        }
        Some(())
    }
    fn segment(&mut self, a: Point, b: Point, sink: &mut Sink<'_>, work: &mut Work<'_>) -> Option<()> {
        work.step()?;
        let length = (b - a).hypot();
        if !length.is_finite() {
            return None;
        }
        let tangent = if length > 0. { Some((b - a) / length) } else { None };
        self.zeros(a, tangent, sink, work)?;
        if length == 0. {
            if self.on() {
                if sink.pending.is_none() {
                    sink.begin_run(a, None)?;
                }
                sink.point(b)?;
            }
            return Some(());
        }
        let mut position = 0.;
        while position < length {
            work.step()?;
            let amount = (length - position).min(self.remaining);
            let next = position + amount;
            if amount <= 0. || next <= position {
                return None;
            }
            let point = if next >= length {
                b
            } else {
                a + (b - a) * (next / length)
            };
            if self.on() {
                if sink.pending.is_none() {
                    sink.begin_run(a + (b - a) * (position / length), tangent)?;
                }
                sink.point(point)?;
            }
            position = next;
            if !self.pattern.is_empty() {
                self.remaining -= amount;
                if self.remaining == 0. {
                    sink.end_run()?;
                    self.advance()?;
                    self.zeros(point, tangent, sink, work)?;
                }
            }
        }
        Some(())
    }
}
fn walk(path: &BezPath, stroke: &Stroke, sink: &mut Sink<'_>, work: &mut Work<'_>) -> Option<()> {
    let pattern = stroke.dash_pattern.as_slice();
    let mut current = None;
    let mut start = None;
    let initial_dash = Dash::reset(pattern, stroke.dash_offset)?;
    let mut dash = initial_dash;
    let mut open = false;
    for &element in path.elements() {
        work.step()?;
        match element {
            PathEl::MoveTo(p) => {
                if open {
                    sink.end_subpath(false)?;
                }
                sink.begin_subpath(p)?;
                dash = initial_dash;
                current = Some(p);
                start = Some(p);
                open = true;
            }
            PathEl::LineTo(p) => {
                if !open {
                    sink.begin_subpath(current?)?;
                    start = current;
                    open = true;
                    dash = initial_dash;
                }
                dash.segment(current?, p, sink, work)?;
                current = Some(p);
            }
            PathEl::ClosePath => {
                if !open {
                    if current.is_none() {
                        return None;
                    }
                    continue;
                }
                if current? != start? {
                    dash.segment(current?, start?, sink, work)?;
                }
                sink.end_subpath(true)?;
                current = start;
                open = false;
            }
            _ => return None,
        }
    }
    if open {
        sink.end_subpath(false)?;
    }
    Some(())
}

/// Flatten and split in original stroke metric, with exact-sized admitted
/// point/run/subpath buffers. The cap refuses dense dashes before plan storage
/// allocation. Width zero is retained for later device-space hairline handling.
pub(super) fn admitted_stroke_runs(
    path: &BezPath,
    stroke: &Stroke,
    transform: Affine,
    device_tolerance: f64,
    max_points: usize,
    max_runs: usize,
    work_limit: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<StrokeRuns> {
    if cancelled()
        || max_points == 0
        || max_points > 1_000_000
        || max_runs == 0
        || max_runs > 1_000_000
        || !stroke.width.is_finite()
        || stroke.width < 0.
        || !stroke.miter_limit.is_finite()
        || stroke.miter_limit < 1.
        || !stroke.dash_offset.is_finite()
        || stroke.dash_pattern.len() > 4096
        || stroke.dash_pattern.iter().any(|v| !v.is_finite() || *v < 0.)
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
        || !device_tolerance.is_finite()
        || device_tolerance <= 0.
    {
        return None;
    }
    let c = transform.as_coeffs();
    let norm = c[0].hypot(c[1]).hypot(c[2].hypot(c[3]));
    if !norm.is_finite() || norm <= 0. {
        return None;
    }
    Dash::reset(stroke.dash_pattern.as_slice(), stroke.dash_offset)?;
    let flat = super::admitted_global_contour(
        path,
        Affine::IDENTITY,
        device_tolerance / norm,
        max_points,
        cancelled,
        admit,
    )?;
    let mut work = Work {
        left: work_limit,
        cancelled,
    };
    let mut sink = Sink {
        points: 0,
        runs: 0,
        subpaths: 0,
        pending: None,
        first_run: 0,
        subpath_start: Point::ZERO,
        max_points,
        max_runs,
        output: None,
    };
    walk(&flat.path, stroke, &mut sink, &mut work)?;
    let (point_count, run_count, subpath_count) = (sink.points, sink.runs, sink.subpaths);
    let bytes = point_count
        .checked_mul(size_of::<Point>())?
        .checked_add(run_count.checked_mul(size_of::<Run>())?)?
        .checked_add(subpath_count.checked_mul(size_of::<Subpath>())?)?;
    let credit = admit(bytes)?;
    if cancelled() {
        return None;
    }
    let mut points = Vec::new();
    points.try_reserve_exact(point_count).ok()?;
    let mut runs = Vec::new();
    runs.try_reserve_exact(run_count).ok()?;
    let mut subpaths = Vec::new();
    subpaths.try_reserve_exact(subpath_count).ok()?;
    let mut output = Sink {
        points: 0,
        runs: 0,
        subpaths: 0,
        pending: None,
        first_run: 0,
        subpath_start: Point::ZERO,
        max_points: point_count,
        max_runs: run_count.max(subpath_count),
        output: Some((&mut points, &mut runs, &mut subpaths)),
    };
    walk(&flat.path, stroke, &mut output, &mut work)?;
    if output.points != point_count || output.runs != run_count || output.subpaths != subpath_count {
        return None;
    }
    drop(output);
    drop(flat);
    Some(StrokeRuns {
        points,
        runs,
        subpaths,
        _credit: credit,
    })
}

#[test]
fn independently_authored_dash_runs_preserve_phase_zero_dots_and_closed_metadata() {
    let mut path = BezPath::new();
    path.move_to((0., 0.));
    path.line_to((12., 0.));
    let stroke = Stroke {
        dash_pattern: [4., 3.].into_iter().collect(),
        dash_offset: 1.,
        ..Stroke::default()
    };
    let plan = admitted_stroke_runs(
        &path,
        &stroke,
        Affine::IDENTITY,
        0.0001,
        10_000,
        10_000,
        100_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    let endpoints: Vec<_> = plan
        .runs
        .iter()
        .map(|r| (plan.points[r.start].x, plan.points[r.start + r.len - 1].x))
        .collect();
    assert_eq!(endpoints, [(0., 3.), (6., 10.)]);
    let dot = Stroke {
        dash_pattern: [0., 2.].into_iter().collect(),
        ..Stroke::default()
    };
    let plan = admitted_stroke_runs(
        &path,
        &dot,
        Affine::IDENTITY,
        0.0001,
        10_000,
        10_000,
        100_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(
        plan.runs
            .iter()
            .map(|r| plan.points[r.start].x)
            .collect::<Vec<_>>(),
        [0., 2., 4., 6., 8., 10., 12.]
    );
    assert!(plan.runs.iter().all(|r| r.len == 1));
    path.close_path();
    let plan = admitted_stroke_runs(
        &path,
        &Stroke::default(),
        Affine::IDENTITY,
        0.0001,
        10_000,
        10_000,
        100_000,
        &|| false,
        &|_| Some(Box::new(())),
    )
    .unwrap();
    assert_eq!(plan.subpaths.len(), 1);
    assert!(plan.subpaths[0].closed);
    assert_eq!(plan.subpaths[0].first_run, 0);
    assert_eq!(plan.subpaths[0].run_count, 1);
}

#[test]
fn dash_plan_refusal_cancellation_and_dense_pattern_release_all_storage() {
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
    path.line_to((12., 0.));
    path.line_to((12., 8.));
    path.close_path();
    let stroke = Stroke {
        dash_pattern: [4., 3.].into_iter().collect(),
        dash_offset: 1.,
        ..Stroke::default()
    };
    let checks = Cell::new(0);
    let admissions = Cell::new(0);
    drop(
        admitted_stroke_runs(
            &path,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            10_000,
            10_000,
            100_000,
            &|| {
                checks.set(checks.get() + 1);
                false
            },
            &|_| {
                admissions.set(admissions.get() + 1);
                Some(Box::new(()))
            },
        )
        .unwrap(),
    );
    for deny in 1..=admissions.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        assert!(
            admitted_stroke_runs(
                &path,
                &stroke,
                Affine::IDENTITY,
                0.0001,
                10_000,
                10_000,
                100_000,
                &|| false,
                &|bytes| {
                    calls.set(calls.get() + 1);
                    if calls.get() == deny {
                        return None;
                    }
                    used.fetch_add(bytes, Ordering::Relaxed);
                    Some(Box::new(Credit(used.clone(), bytes)))
                }
            )
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        assert!(
            admitted_stroke_runs(
                &path,
                &stroke,
                Affine::IDENTITY,
                0.0001,
                10_000,
                10_000,
                100_000,
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
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    let dense = Stroke {
        dash_pattern: [0.000001, 0.000001].into_iter().collect(),
        ..Stroke::default()
    };
    let calls = Cell::new(0);
    let used = Arc::new(AtomicUsize::new(0));
    assert!(
        admitted_stroke_runs(
            &path,
            &dense,
            Affine::IDENTITY,
            0.0001,
            100,
            100,
            10_000,
            &|| false,
            &|bytes| {
                calls.set(calls.get() + 1);
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            }
        )
        .is_none()
    );
    assert_eq!(calls.get(), 1, "dense dash refused before run-plan admission");
    assert_eq!(used.load(Ordering::Relaxed), 0);
    let used = Arc::new(AtomicUsize::new(0));
    assert!(
        admitted_stroke_runs(
            &path,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            10_000,
            10_000,
            1,
            &|| false,
            &|bytes| {
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            }
        )
        .is_none()
    );
    assert_eq!(
        used.load(Ordering::Relaxed),
        0,
        "run work-budget refusal releases flattened input"
    );
    let invalid = Stroke {
        dash_pattern: [0., 0.].into_iter().collect(),
        ..Stroke::default()
    };
    assert!(
        admitted_stroke_runs(
            &path,
            &invalid,
            Affine::IDENTITY,
            0.0001,
            100,
            100,
            10_000,
            &|| false,
            &|_| panic!("zero-period dash admitted")
        )
        .is_none()
    );
}

#[test]
fn odd_dash_cycle_and_negative_offset_match_independent_intervals() {
    let mut path = BezPath::new();
    path.move_to((0., 0.));
    path.line_to((12., 0.));
    for (offset, expected) in [
        (0., vec![(0., 2.), (3., 4.), (6., 7.), (8., 10.), (11., 12.)]),
        (-1., vec![(1., 3.), (4., 5.), (7., 8.), (9., 11.)]),
    ] {
        let stroke = Stroke {
            dash_pattern: [2., 1., 1.].into_iter().collect(),
            dash_offset: offset,
            ..Stroke::default()
        };
        let plan = admitted_stroke_runs(
            &path,
            &stroke,
            Affine::IDENTITY,
            0.0001,
            10_000,
            10_000,
            100_000,
            &|| false,
            &|_| Some(Box::new(())),
        )
        .unwrap();
        assert_eq!(
            plan.runs
                .iter()
                .map(|r| (plan.points[r.start].x, plan.points[r.start + r.len - 1].x))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn curved_run_keeps_authored_endpoints_and_retains_only_plan_credit() {
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
    path.curve_to((0., 8.), (16., 0.), (16., 8.));
    let used = Arc::new(AtomicUsize::new(0));
    let plan = admitted_stroke_runs(
        &path,
        &Stroke::default(),
        Affine::scale_non_uniform(2., 3.),
        0.0001,
        10_000,
        10_000,
        100_000,
        &|| false,
        &|bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)))
        },
    )
    .unwrap();
    assert_eq!(plan.runs.len(), 1);
    assert!(plan.points.len() > 2);
    assert_eq!(plan.points.first(), Some(&Point::new(0., 0.)));
    assert_eq!(plan.points.last(), Some(&Point::new(16., 8.)));
    assert_eq!(
        used.load(Ordering::Relaxed),
        plan.points.len() * size_of::<Point>()
            + plan.runs.len() * size_of::<Run>()
            + plan.subpaths.len() * size_of::<Subpath>()
    );
    drop(plan);
    assert_eq!(used.load(Ordering::Relaxed), 0);
}
