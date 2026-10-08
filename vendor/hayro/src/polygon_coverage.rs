//! Test-only globally anchored polygon area kernel. Floating-point area, not a
//! certified exact predicate implementation. Curves must be prepared separately.
use kurbo::{BezPath, PathEl, Point};
use vello_cpu::peniko::Fill;

#[derive(Clone, Copy)]
pub(super) struct Edge {
    a: Point,
    b: Point,
    region: usize,
}
impl Edge {
    fn low(self) -> f64 {
        self.a.y.min(self.b.y)
    }
    fn high(self) -> f64 {
        self.a.y.max(self.b.y)
    }
    fn slope(self) -> f64 {
        if self.a.y == self.b.y {
            0.
        } else {
            (self.b.x - self.a.x) / (self.b.y - self.a.y)
        }
    }
    fn x(self, y: f64) -> f64 {
        self.a.x + (y - self.a.y) * self.slope()
    }
    fn winding(self) -> i32 {
        if self.b.y > self.a.y { 1 } else { -1 }
    }
}
struct Work<'a> {
    left: usize,
    cancelled: &'a dyn Fn() -> bool,
}
impl Work<'_> {
    fn step(&mut self, count: usize) -> Option<()> {
        if (self.cancelled)() {
            return None;
        }
        self.left = match self.left.checked_sub(count) {
            Some(left) => left,
            None => {
                if std::env::var_os("RRRAH_TRACE_NATIVE_AREA_REFUSAL").is_some() {
                    eprintln!("POLYGON_REFUSAL work_left={} requested={count}", self.left);
                }
                return None;
            }
        };
        Some(())
    }
}

/// Intersect filled polygon regions with one global unit pixel. Both edge
/// storage and scratch are admitted before allocation; every loop is bounded
/// by caller work and cancellation. No viewport-local coordinate conversion.
/// Refuses more than 64 regions or 4096 row-active edges, including interior horizontal segments.
pub(super) fn polygon_pixel_area(
    regions: &[(&BezPath, Fill)],
    x: i32,
    y: i32,
    work_limit: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<f64> {
    if regions.is_empty() || regions.len() > 64 || x.abs_diff(0) > 1_048_576 || y.abs_diff(0) > 1_048_576 {
        return None;
    }
    let mut work = Work {
        left: work_limit,
        cancelled,
    };
    work.step(1)?;
    let row = prepare_row(regions, y, &mut work, admit)?;
    row.pixel_area_with_work(regions, x, y, &mut work, admit)
}

pub(super) struct PolygonRow {
    edges: Vec<Edge>,
    crossings: Vec<(f64, f64)>, // Sorted (global X, global Y).
    y: i32,
    regions: usize,
    // Retain credit until all prepared edge storage has dropped.
    _credit: Box<dyn std::any::Any + Send + Sync>,
    _crossing_credit: Box<dyn std::any::Any + Send + Sync>,
}

pub(super) fn admitted_polygon_row(
    regions: &[(&BezPath, Fill)],
    y: i32,
    work_limit: usize,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PolygonRow> {
    if regions.is_empty() || regions.len() > 64 || y.abs_diff(0) > 1_048_576 {
        return None;
    }
    let mut work = Work {
        left: work_limit,
        cancelled,
    };
    prepare_row(regions, y, &mut work, admit)
}

fn prepare_row(
    regions: &[(&BezPath, Fill)],
    y: i32,
    work: &mut Work<'_>,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PolygonRow> {
    let cancelled = work.cancelled;
    let (top, bottom) = (f64::from(y), f64::from(y) + 1.);
    let mut walk = |out: &mut dyn FnMut(Edge) -> Option<()>| -> Option<()> {
        for (region, (path, _)) in regions.iter().enumerate() {
            let mut current = None;
            let mut start = None;
            for &element in path.elements() {
                work.step(1)?;
                let point = match element {
                    PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p),
                    PathEl::ClosePath => None,
                    _ => return None,
                };
                if let Some(p) = point {
                    if !p.x.is_finite()
                        || !p.y.is_finite()
                        || p.x.abs() > 1_048_576.
                        || p.y.abs() > 1_048_576.
                    {
                        return None;
                    }
                }
                let mut edge = |a: Point, b: Point| -> Option<()> {
                    if (a.y != b.y && a.y.min(b.y) < bottom && a.y.max(b.y) > top)
                        || (a.y == b.y && a.x != b.x && a.y > top && a.y < bottom)
                    {
                        let edge = Edge { a, b, region };
                        if !edge.slope().is_finite() {
                            return None;
                        }
                        out(edge)?;
                    }
                    Some(())
                };
                match element {
                    PathEl::MoveTo(p) => {
                        if let (Some(a), Some(b)) = (current, start) {
                            edge(a, b)?;
                        }
                        current = Some(p);
                        start = Some(p);
                    }
                    PathEl::LineTo(p) => {
                        edge(current?, p)?;
                        current = Some(p);
                    }
                    PathEl::ClosePath => {
                        edge(current?, start?)?;
                        current = start;
                    }
                    _ => unreachable!(),
                }
            }
            if let (Some(a), Some(b)) = (current, start) {
                if a != b {
                    out_if_active(a, b, region, top, bottom, out)?;
                }
            }
        }
        Some(())
    };
    let mut count = 0usize;
    walk(&mut |_| {
        count = count.checked_add(1)?;
        if count > 4096 {
            if std::env::var_os("RRRAH_TRACE_NATIVE_AREA_REFUSAL").is_some() {
                eprintln!("POLYGON_REFUSAL active_edges={count}");
            }
            None
        } else {
            Some(())
        }
    })?;
    let edge_credit = admit(count.checked_mul(size_of::<Edge>())?)?;
    if cancelled() {
        return None;
    }
    let mut edges = Vec::new();
    edges.try_reserve_exact(count).ok()?;
    walk(&mut |edge| {
        edges.push(edge);
        Some(())
    })?;
    drop(walk);
    work.step(edges.len().checked_mul(16)?)?;
    edges.sort_unstable_by(|a, b| a.a.x.min(a.b.x).total_cmp(&b.a.x.min(b.b.x)));
    let mut crossing_count = 0usize;
    visit_crossings(&edges, top, bottom, work, &mut |_| {
        crossing_count = crossing_count.checked_add(1)?;
        if crossing_count > 1_000_000 {
            None
        } else {
            Some(())
        }
    })?;
    let crossing_credit = admit(crossing_count.checked_mul(size_of::<(f64, f64)>())?)?;
    if cancelled() {
        return None;
    }
    let mut crossings = Vec::new();
    crossings.try_reserve_exact(crossing_count).ok()?;
    visit_crossings(&edges, top, bottom, work, &mut |point| {
        if crossings.len() >= crossing_count {
            return None;
        }
        crossings.push(point);
        Some(())
    })?;
    if crossings.len() != crossing_count {
        return None;
    }
    work.step(crossings.len().checked_mul(16)?)?;
    crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    Some(PolygonRow {
        edges,
        crossings,
        y,
        regions: regions.len(),
        _credit: edge_credit,
        _crossing_credit: crossing_credit,
    })
}

impl PolygonRow {
    pub(super) fn pixel_area(
        &self,
        regions: &[(&BezPath, Fill)],
        x: i32,
        y: i32,
        work_limit: usize,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Option<f64> {
        if regions.is_empty() || regions.len() > 64 || x.abs_diff(0) > 1_048_576 || y.abs_diff(0) > 1_048_576
        {
            return None;
        }
        let mut work = Work {
            left: work_limit,
            cancelled,
        };
        work.step(1)?;
        self.pixel_area_with_work(regions, x, y, &mut work, admit)
    }

    fn pixel_area_with_work(
        &self,
        regions: &[(&BezPath, Fill)],
        x: i32,
        y: i32,
        work: &mut Work<'_>,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Option<f64> {
        if y != self.y || regions.len() != self.regions {
            return None;
        }
        let cancelled = work.cancelled;
        let edges = &self.edges;
        let count = edges.len();
        if count == 0 {
            return Some(0.);
        }
        let (left, right, top, bottom) = (f64::from(x), f64::from(x) + 1., f64::from(y), f64::from(y) + 1.);
        let maximum_cuts = 2usize
            .checked_add(count.checked_mul(4)?)?
            .checked_add(count.checked_mul(count - 1)?.checked_div(2)?)?;
        let mut cut_count = 0usize;
        visit_cuts(
            &edges,
            &self.crossings,
            (left, right, top, bottom),
            work,
            &mut |_| {
                cut_count = cut_count.checked_add(1)?;
                if cut_count > maximum_cuts { None } else { Some(()) }
            },
        )?;
        let bytes = cut_count
            .checked_mul(size_of::<f64>())?
            .checked_add(count.checked_mul(size_of::<(f64, usize)>())?)?
            .checked_add(regions.len().checked_mul(size_of::<i32>())?)?;
        let scratch_credit = admit(bytes)?;
        if cancelled() {
            return None;
        }
        let mut cuts = Vec::new();
        cuts.try_reserve_exact(cut_count).ok()?;
        let mut hits = Vec::new();
        hits.try_reserve_exact(count).ok()?;
        let mut winding = Vec::new();
        winding.try_reserve_exact(regions.len()).ok()?;
        winding.resize(regions.len(), 0i32);
        visit_cuts(
            &edges,
            &self.crossings,
            (left, right, top, bottom),
            work,
            &mut |value| {
                if cuts.len() >= cut_count {
                    return None;
                }
                cuts.push(value);
                Some(())
            },
        )?;
        if cuts.len() != cut_count {
            return None;
        }
        work.step(cuts.len().checked_mul(32)?)?;
        cuts.sort_unstable_by(f64::total_cmp);
        cuts.dedup();
        let mut area = 0.;
        let mut compensation = 0.;
        for slab in cuts.windows(2) {
            work.step(1)?;
            let (a, b) = (slab[0], slab[1]);
            if b <= a {
                return None;
            }
            // Consecutive representable Y values can have no representable midpoint.
            // Evaluate linear X at both endpoints instead of inventing/dropping a slab.

            hits.clear();
            winding.fill(0);
            for (index, &edge) in edges.iter().enumerate() {
                work.step(1)?;
                if !(edge.low() <= a && edge.high() >= b) {
                    // Outside vertices need no slab cuts. Their closed-contour
                    // winding at the left border is constant until a border
                    // crossing, which is still a cut. Sample the slab interior
                    // with half-open edge lifetime, consistently with full edges.
                    // Adjacent representable Y coordinates fall back to the top.
                    let midpoint = a * 0.5 + b * 0.5;
                    let sample_y = if midpoint < b { midpoint } else { a };
                    if edge.low() <= sample_y && edge.high() > sample_y && edge.x(sample_y) <= left {
                        winding[edge.region] += edge.winding();
                    }
                    continue;
                }
                if edge.low() <= a && edge.high() >= b {
                    let x = edge.x(a) * 0.5 + edge.x(b) * 0.5;
                    if !x.is_finite() {
                        return None;
                    }
                    if x <= left {
                        winding[edge.region] += edge.winding();
                    } else if x < right {
                        hits.push((x, index));
                    }
                }
            }
            work.step(hits.len().checked_mul(16)?)?;
            hits.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            // Edges wholly left of this slab contribute only the initial winding;
            // edges wholly right cannot affect the pixel. Sort only interior hits.
            let mut previous = None;
            for next in hits.iter().map(|hit| Some(hit.1)).chain(std::iter::once(None)) {
                work.step(regions.len() + 1)?;
                let inside = regions.iter().enumerate().all(|(r, (_, rule))| match rule {
                    Fill::NonZero => winding[r] != 0,
                    Fill::EvenOdd => winding[r] % 2 != 0,
                });
                if inside {
                    let xa = previous.map_or(left, |index: usize| edges[index].x(a).max(left));
                    let xb = previous.map_or(left, |index: usize| edges[index].x(b).max(left));
                    let ya = next.map_or(right, |index| edges[index].x(a).min(right));
                    let yb = next.map_or(right, |index| edges[index].x(b).min(right));
                    let contribution = ((ya - xa).max(0.) + (yb - xb).max(0.)) * 0.5 * (b - a);
                    if !contribution.is_finite() {
                        return None;
                    }
                    let corrected = contribution - compensation;
                    let sum = area + corrected;
                    compensation = (sum - area) - corrected;
                    area = sum;
                }
                if let Some(index) = next {
                    let edge = edges[index];
                    winding[edge.region] += edge.winding();
                }
                previous = next;
            }
        }
        if cancelled() || !area.is_finite() || !(0. ..=1. + 1e-12).contains(&area) {
            if std::env::var_os("RRRAH_TRACE_NATIVE_AREA_REFUSAL").is_some() {
                eprintln!("POLYGON_REFUSAL area={area}");
            }
            return None;
        }
        drop(winding);
        drop(hits);
        drop(cuts);
        drop(scratch_credit);
        Some(area.clamp(0., 1.))
    }
}

/// Count and then emit the same subdivision cuts. Disjoint segment X ranges
/// cannot cross, so they need no line-intersection calculation.
fn visit_cuts(
    edges: &[Edge],
    crossings: &[(f64, f64)],
    bounds: (f64, f64, f64, f64),
    work: &mut Work<'_>,
    out: &mut dyn FnMut(f64) -> Option<()>,
) -> Option<()> {
    let (left, right, top, bottom) = bounds;
    work.step(1)?;
    out(top)?;
    out(bottom)?;
    let mut add = |value: f64| -> Option<()> {
        if value.is_finite() && value > top && value < bottom {
            out(value)?;
        }
        Some(())
    };
    for &edge in edges {
        work.step(1)?;
        if edge.a.y == edge.b.y {
            if edge.a.x.min(edge.b.x) <= right && edge.a.x.max(edge.b.x) >= left {
                add(edge.a.y)?;
            }
            continue;
        }
        if edge.a.x >= left && edge.a.x <= right {
            add(edge.a.y)?;
        }
        if edge.b.x >= left && edge.b.x <= right {
            add(edge.b.y)?;
        }
        let lo = edge.low().max(top);
        let hi = edge.high().min(bottom);
        let slope = edge.slope();
        if slope != 0. {
            for border in [left, right] {
                let at = lo + (border - edge.x(lo)) / slope;
                if at > lo && at < hi {
                    add(at)?;
                }
            }
        }
    }
    work.step(32)?; // Two bounded binary searches over <=1M crossings.
    let first = crossings.partition_point(|p| p.0 < left);
    let last = crossings.partition_point(|p| p.0 <= right);
    for &(_, y) in &crossings[first..last] {
        work.step(1)?;
        add(y)?;
    }
    Some(())
}

fn visit_crossings(
    edges: &[Edge],
    top: f64,
    bottom: f64,
    work: &mut Work<'_>,
    out: &mut dyn FnMut((f64, f64)) -> Option<()>,
) -> Option<()> {
    for (index, &edge) in edges.iter().enumerate() {
        work.step(1)?;
        let lo = edge.low().max(top);
        let hi = edge.high().min(bottom);
        let slope = edge.slope();
        for &other in &edges[index + 1..] {
            work.step(1)?;
            if edge.a.x.max(edge.b.x) < other.a.x.min(other.b.x) {
                break;
            }
            let lo = lo.max(other.low());
            let hi = hi.min(other.high());
            let denominator = slope - other.slope();
            if lo < hi && denominator != 0. {
                let y = lo + (other.x(lo) - edge.x(lo)) / denominator;
                if y > lo && y < hi {
                    let x = edge.x(y);
                    if !x.is_finite() {
                        return None;
                    }
                    out((x, y))?;
                }
            }
        }
    }
    Some(())
}

fn out_if_active(
    a: Point,
    b: Point,
    region: usize,
    top: f64,
    bottom: f64,
    out: &mut dyn FnMut(Edge) -> Option<()>,
) -> Option<()> {
    if (a.y != b.y && a.y.min(b.y) < bottom && a.y.max(b.y) > top)
        || (a.y == b.y && a.x != b.x && a.y > top && a.y < bottom)
    {
        let edge = Edge { a, b, region };
        if !edge.slope().is_finite() {
            return None;
        }
        out(edge)?;
    }
    Some(())
}

#[test]
fn independent_polygon_areas_cover_winding_holes_overlap_and_clipping() {
    use kurbo::Shape;
    let rectangle = |a, b, c, d| kurbo::Rect::new(a, b, c, d).to_path(0.1);
    let mut triangle = BezPath::new();
    triangle.move_to((0., 0.));
    triangle.line_to((1., 0.));
    triangle.line_to((0., 1.));
    let mut hole = rectangle(0., 0., 1., 1.);
    hole.extend(rectangle(0.25, 0.25, 0.75, 0.75).elements().iter().copied());
    let mut overlap = rectangle(0., 0., 0.75, 1.);
    overlap.extend(rectangle(0.25, 0., 1., 1.).elements().iter().copied());
    let half = rectangle(0., 0., 1., 0.5);
    for (path, rule, expected) in [
        (&triangle, Fill::NonZero, 0.5),
        (&hole, Fill::EvenOdd, 0.75),
        (&hole, Fill::NonZero, 1.),
        (&overlap, Fill::NonZero, 1.),
        (&overlap, Fill::EvenOdd, 0.5),
    ] {
        assert_eq!(
            polygon_pixel_area(&[(path, rule)], 0, 0, 1_000_000, &|| false, &|_| Some(Box::new(
                ()
            ))),
            Some(expected)
        );
    }
    assert_eq!(
        polygon_pixel_area(
            &[(&triangle, Fill::NonZero), (&half, Fill::NonZero)],
            0,
            0,
            1_000_000,
            &|| false,
            &|_| Some(Box::new(()))
        ),
        Some(0.375)
    );
}

#[test]
fn polygon_area_refusal_cancellation_and_work_exhaustion_release_scratch() {
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
    let mut bowtie = BezPath::new();
    for (i, p) in [(0., 0.), (1., 1.), (0., 1.), (1., 0.)].into_iter().enumerate() {
        if i == 0 {
            bowtie.move_to(p);
        } else {
            bowtie.line_to(p);
        }
    }
    let checks = Cell::new(0);
    let admissions = Cell::new(0);
    let regions = [(&bowtie, Fill::EvenOdd)];
    assert_eq!(
        polygon_pixel_area(
            &regions,
            0,
            0,
            1_000_000,
            &|| {
                checks.set(checks.get() + 1);
                false
            },
            &|_| {
                admissions.set(admissions.get() + 1);
                Some(Box::new(()))
            }
        ),
        Some(0.5)
    );
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        let value = polygon_pixel_area(
            &regions,
            0,
            0,
            1_000_000,
            &|| {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            },
            &|bytes| {
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            },
        );
        assert!(value.is_none(), "cancel={stop}");
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for deny in 1..=admissions.get() {
        let calls = Cell::new(0);
        let used = Arc::new(AtomicUsize::new(0));
        assert!(
            polygon_pixel_area(&regions, 0, 0, 1_000_000, &|| false, &|bytes| {
                calls.set(calls.get() + 1);
                if calls.get() == deny {
                    return None;
                }
                used.fetch_add(bytes, Ordering::Relaxed);
                Some(Box::new(Credit(used.clone(), bytes)))
            })
            .is_none()
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    for budget in 0..256 {
        let used = Arc::new(AtomicUsize::new(0));
        let value = polygon_pixel_area(&regions, 0, 0, budget, &|| false, &|bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)))
        });
        assert_eq!(used.load(Ordering::Relaxed), 0);
        if let Some(area) = value {
            assert_eq!(area, 0.5);
        }
    }
    let mut invalid = BezPath::new();
    invalid.move_to((f64::NAN, 0.));
    assert!(
        polygon_pixel_area(
            &[(&invalid, Fill::NonZero)],
            0,
            0,
            1_000_000,
            &|| false,
            &|_| panic!("invalid geometry admitted")
        )
        .is_none()
    );
}

#[test]
fn polygon_area_global_translation_preserves_independent_fractional_reference() {
    let mut source = BezPath::new();
    source.move_to((0., 0.));
    source.line_to((1., 0.));
    source.line_to((0., 1.));
    for (x, y) in [(0, 0), (4096, 8192), (-4096, -8192), (1048575, -1048575)] {
        let path = kurbo::Affine::translate((f64::from(x), f64::from(y))) * &source;
        assert_eq!(
            polygon_pixel_area(&[(&path, Fill::NonZero)], x, y, 1_000_000, &|| false, &|_| Some(
                Box::new(())
            )),
            Some(0.5)
        );
    }
}

#[test]
fn polygon_active_edge_exhaustion_refuses_before_admission() {
    let mut path = BezPath::new();
    for _ in 0..4097 {
        path.move_to((0., 0.));
        path.line_to((1., 1.));
    }
    assert!(
        polygon_pixel_area(
            &[(&path, Fill::NonZero)],
            0,
            0,
            1_000_000,
            &|| false,
            &|_| panic!("excessive edges admitted")
        )
        .is_none()
    );
}

#[test]
fn adjacent_representable_scanline_cuts_retain_independent_rectangle_area() {
    use kurbo::Shape;
    let a = 0.5f64;
    let b = f64::from_bits(a.to_bits() + 1);
    let path = kurbo::Rect::new(0., a, 1., b).to_path(0.1);
    let area = polygon_pixel_area(&[(&path, Fill::NonZero)], 0, 0, 1_000_000, &|| false, &|_| {
        Some(Box::new(()))
    })
    .unwrap();
    assert_eq!(area, b - a);
    assert!(area > 0.);
}

#[test]
fn admitted_row_reuses_edges_and_retains_credit_across_pixels() {
    use kurbo::Shape;
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
    let used = Arc::new(AtomicUsize::new(0));
    let admit = |bytes| {
        used.fetch_add(bytes, Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
    };
    let path = kurbo::Rect::new(0.25, 0., 1.75, 1.).to_path(0.1);
    let regions = [(&path, Fill::NonZero)];
    let row = admitted_polygon_row(&regions, 0, 1_000_000, &|| false, &admit).unwrap();
    let retained = used.load(Ordering::Relaxed);
    assert_eq!(retained, 2 * size_of::<Edge>());
    for (x, expected) in [(-1, 0.), (0, 0.75), (1, 0.75), (2, 0.)] {
        assert_eq!(
            row.pixel_area(&regions, x, 0, 1_000_000, &|| false, &admit),
            Some(expected)
        );
        assert_eq!(used.load(Ordering::Relaxed), retained);
    }
    assert!(
        row.pixel_area(&regions, 0, 1, 1_000_000, &|| false, &admit)
            .is_none()
    );
    assert!(
        row.pixel_area(&regions, 0, 0, 1_000_000, &|| true, &admit)
            .is_none()
    );
    assert_eq!(used.load(Ordering::Relaxed), retained);
    drop(row);
    assert_eq!(used.load(Ordering::Relaxed), 0);
}

#[test]
fn outside_vertices_preserve_winding_and_border_crossings() {
    let admit = |_: usize| Some(Box::new(()) as Box<dyn std::any::Any + Send + Sync>);
    for points in [
        vec![(-2., 0.), (1., 0.), (1., 1.), (-2., 1.), (-3., 0.5)],
        vec![
            (-2., 0.),
            (1., 0.),
            (1., 1.),
            (-2., 1.),
            (-3., 0.25),
            (-2.5, 0.125),
        ],
    ] {
        let mut path = BezPath::new();
        path.move_to(points[0]);
        for point in &points[1..] {
            path.line_to(*point);
        }
        path.close_path();
        for rule in [Fill::NonZero, Fill::EvenOdd] {
            assert_eq!(
                polygon_pixel_area(&[(&path, rule)], 0, 0, 1_000_000, &|| false, &admit),
                Some(1.)
            );
        }
    }
    // The left boundary changes edges outside the pixel before entering it.
    // Independent trapezoid areas: 1/4 + 1/3 + 1/12 + 1/24 + 1/6 = 7/8.
    let mut notch = BezPath::new();
    notch.move_to((-2., 0.));
    for point in [(-2., 0.25), (1., 0.75), (-2., 1.), (2., 1.), (2., 0.)] {
        notch.line_to(point);
    }
    notch.close_path();
    for rule in [Fill::NonZero, Fill::EvenOdd] {
        let area = polygon_pixel_area(&[(&notch, rule)], 0, 0, 1_000_000, &|| false, &admit).unwrap();
        assert!((area - 0.875).abs() < 1e-14, "{area}");
    }
    // A diagonal starts exactly on the left border; its winding must be
    // applied at its interior hit rather than counted twice at the border.
    let mut triangle = BezPath::new();
    triangle.move_to((0., 0.));
    triangle.line_to((1., 1.));
    triangle.line_to((0., 1.));
    triangle.close_path();
    assert_eq!(
        polygon_pixel_area(&[(&triangle, Fill::NonZero)], 0, 0, 1_000_000, &|| false, &admit),
        Some(0.5)
    );
}
