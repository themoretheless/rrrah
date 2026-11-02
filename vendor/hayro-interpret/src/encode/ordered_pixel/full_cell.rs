//! Sufficient nonoverlap and full-coverage proofs for positive clipped polygons.
//!
//! Opposite internal edges cancel exactly. If the remaining directed edges
//! traverse each unit border once, their winding is one inside the square and
//! zero outside. Every input polygon is convex and counterclockwise, so its
//! nonnegative winding is its coverage: no positive-area overlaps or holes
//! remain. Affine source integrals can then be added without an owner overlay.
//! For partial coverage a separate sufficient proof bounds the surviving
//! boundary to at most two crossings of any generic axis-aligned line. Since
//! all input winding is nonnegative and vanishes outside the unit square,
//! coverage is then zero or one. Its measured area is preserved, including gaps.
use super::{Admitter, PixelPartitionError, PixelSourceOwner, Polygon, Result, sort_by, tick};
use kurbo::Point;

const MAX_TRIANGLES: usize = 4096;
const CLIPPED_VERTICES: usize = 7;

#[derive(Clone, Copy)]
struct BoundaryEdge {
    key: [u64; 4],
    direction: i8,
}

fn point_key(point: Point) -> Option<[u64; 2]> {
    let coordinates = [point.x, point.y];
    coordinates
        .iter()
        .all(|v| v.is_finite() && (0. ..=1.).contains(v))
        .then(|| coordinates.map(|v| if v == 0. { 0 } else { v.to_bits() }))
}

fn border(edge: [u64; 4]) -> Option<(usize, u64, u64, i32)> {
    let [ax, ay, bx, by] = edge;
    let one = 1f64.to_bits();
    if ay == 0 && by == 0 {
        Some((0, ax, bx, 1))
    } else if ax == one && bx == one {
        Some((1, ay, by, 1))
    } else if ay == one && by == one {
        Some((2, ax, bx, -1))
    } else if ax == 0 && bx == 0 {
        Some((3, ay, by, -1))
    } else {
        None
    }
}

pub(crate) fn integrate_disjoint_unit_mesh(
    triangles: impl ExactSizeIterator<Item = Result<([Point; 3], PixelSourceOwner)>>,
    admit: &Admitter<'_>,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<[f64; 5]>> {
    integrate_mesh(triangles, admit, work, cancelled, false)
}
#[cfg(test)]
fn integrate_full_unit_mesh(
    triangles: impl ExactSizeIterator<Item = Result<([Point; 3], PixelSourceOwner)>>,
    admit: &Admitter<'_>,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<[f64; 5]>> {
    integrate_mesh(triangles, admit, work, cancelled, true)
}

// At most two surviving edges crossing a generic axis-aligned ray imply
// winding magnitude at most one. Positive input polygons exclude negative
// winding, hence their sum has no overlaps. Gaps keep their measured area.
fn at_most_two_crossings(
    edges: &[BoundaryEdge],
    axis: usize,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool> {
    let mut active = [0u64; 2];
    let mut count = 0;
    for edge in edges {
        tick(work, cancelled)?;
        let a = edge.key[axis];
        let b = edge.key[axis + 2];
        let (from, to) = (a.min(b), a.max(b));
        if from == to {
            continue;
        }
        let mut retained = 0;
        for i in 0..count {
            if active[i] > from {
                active[retained] = active[i];
                retained += 1;
            }
        }
        count = retained;
        if count == 2 {
            return Ok(false);
        }
        active[count] = to;
        count += 1;
    }
    Ok(true)
}
fn full_boundary(edges: &[BoundaryEdge], work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<bool> {
    let mut end = [0u64; 4];
    for edge in edges {
        tick(work, cancelled)?;
        let Some((side, from, to, expected)) = border(edge.key) else {
            return Ok(false);
        };
        if i32::from(edge.direction) != expected || from != end[side] || to <= from {
            return Ok(false);
        }
        end[side] = to;
    }
    Ok(end == [1f64.to_bits(); 4])
}
fn integrate_mesh(
    mut triangles: impl ExactSizeIterator<Item = Result<([Point; 3], PixelSourceOwner)>>,
    admit: &Admitter<'_>,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
    require_full: bool,
) -> Result<Option<[f64; 5]>> {
    let count = triangles.len();
    if count == 0 || count > MAX_TRIANGLES {
        return Ok(None);
    }
    tick(work, cancelled)?;
    let capacity = count
        .checked_mul(CLIPPED_VERTICES)
        .ok_or(PixelPartitionError::Limit)?;
    let bytes = capacity
        .checked_mul(size_of::<BoundaryEdge>())
        .ok_or(PixelPartitionError::Limit)?;
    // Admit while the caller's source and candidate allocations remain live.
    let _credit = admit(bytes).ok_or(PixelPartitionError::Admission)?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(capacity)
        .map_err(|_| PixelPartitionError::Admission)?;
    if edges.capacity() > capacity {
        return Err(PixelPartitionError::Admission);
    }
    let mut integral = [0.; 5];
    let mut correction = [0.; 5];
    for _ in 0..count {
        tick(work, cancelled)?;
        let (mut points, owner) = triangles.next().ok_or(PixelPartitionError::Invalid)??;
        if !owner.valid() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(PixelPartitionError::Invalid);
        }
        let orientation = super::orientation(points[0], points[1], points[2]);
        if orientation == 0 {
            continue;
        }
        if orientation < 0 {
            points.swap(1, 2);
        }
        let mut polygon = Polygon::empty();
        for point in points {
            polygon.push(point)?;
        }
        // Unknown support tags keep the ordinary convex geometric clipper.
        for line in [[0., 1., 0.], [1., -1., 0.], [0., 0., 1.], [1., 0., -1.]] {
            polygon = polygon.clip(line, work, cancelled)?;
        }
        let Some((area, center)) = polygon.moments() else {
            continue;
        };
        if polygon.len > CLIPPED_VERTICES {
            return Ok(None);
        }
        let Some(values) = owner.evaluate(center) else {
            return Ok(None);
        };
        let value = [
            area,
            area * values[0],
            area * values[1],
            area * values[2],
            area * values[3],
        ];
        for k in 0..5 {
            let y = value[k] - correction[k];
            let next = integral[k] + y;
            correction[k] = (next - integral[k]) - y;
            integral[k] = next;
        }
        for [from, to] in polygon.edges() {
            tick(work, cancelled)?;
            let (Some(a), Some(b)) = (point_key(from), point_key(to)) else {
                return Ok(None);
            };
            if a == b {
                continue;
            }
            let (a, b, direction) = if a < b { (a, b, 1) } else { (b, a, -1) };
            if edges.len() == capacity {
                return Err(PixelPartitionError::Limit);
            }
            edges.push(BoundaryEdge {
                key: [a[0], a[1], b[0], b[1]],
                direction,
            });
        }
    }
    // This is only an eligibility hint. Exact boundary cancellation below is
    // mandatory even for a one-ulp gap/overlap or equal overlap and hole areas.
    if integral.iter().any(|v| !v.is_finite())
        || integral[0] < 0.
        || integral[0] > 1. + 1e-9
        || (require_full && (integral[0] - 1.).abs() > 1e-9)
    {
        return Ok(None);
    }
    sort_by(&mut edges, work, cancelled, |a, b| a.key.cmp(&b.key))?;
    let mut position = 0;
    let mut retained = 0;
    while position < edges.len() {
        tick(work, cancelled)?;
        let key = edges[position].key;
        let mut winding = 0i32;
        while position < edges.len() && edges[position].key == key {
            tick(work, cancelled)?;
            winding += i32::from(edges[position].direction);
            position += 1;
        }
        if winding == 0 {
            continue;
        }
        if winding.abs() != 1 {
            return Ok(None);
        }
        edges[retained] = BoundaryEdge {
            key,
            direction: winding as i8,
        };
        retained += 1;
    }
    edges.truncate(retained);
    if full_boundary(&edges, work, cancelled)? {
        // Full coverage follows only from exact unit-border winding.
        integral[0] = 1.;
        return Ok(Some(integral));
    }
    if require_full {
        return Ok(None);
    }
    // Canonical endpoint keys are already ordered by the smaller x endpoint.
    if !at_most_two_crossings(&edges, 0, work, cancelled)? {
        sort_by(&mut edges, work, cancelled, |a, b| {
            let interval = |edge: &BoundaryEdge| (edge.key[1].min(edge.key[3]), edge.key[1].max(edge.key[3]));
            interval(a).cmp(&interval(b))
        })?;
        if !at_most_two_crossings(&edges, 1, work, cancelled)? {
            return Ok(None);
        }
    }
    // Partial coverage retains its actual compensated area; no normalization.
    Ok(Some(integral))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::ordered_pixel::Credit;
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    type Input = ([Point; 3], PixelSourceOwner);
    fn owner(patch_index: usize, blue: bool) -> PixelSourceOwner {
        PixelSourceOwner {
            patch_index,
            sample_index: 0,
            u: [0., 1., 0.],
            v: [0., 0., 1.],
            components: if blue {
                [[0.; 3], [0.; 3], [0., 0., 1.], [0.; 3]]
            } else {
                [[0., 1., 0.], [0.; 3], [0.; 3], [0.; 3]]
            },
            component_count: 3,
        }
    }
    fn triangle(points: [[f64; 2]; 3], owner: PixelSourceOwner) -> Input {
        (points.map(|p| Point::new(p[0], p[1])), owner)
    }
    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Input> {
        vec![
            triangle([[x0, y0], [x1, y0], [x0, y1]], owner(0, false)),
            triangle([[x1, y0], [x1, y1], [x0, y1]], owner(1, true)),
        ]
    }
    fn run(
        inputs: &[Input],
        admit: &Admitter<'_>,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<[f64; 5]>> {
        integrate_full_unit_mesh(inputs.iter().copied().map(Ok), admit, work, cancelled)
    }

    #[test]
    fn full_boundary_proves_coverage_and_affine_color_independent_of_priority_and_winding() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut inputs = rectangle(0., 0., 1., 1.);
        for reverse in [false, true] {
            if reverse {
                inputs.reverse();
                for (points, _) in &mut inputs {
                    points.swap(1, 2);
                }
            }
            let result = run(&inputs, &admit, &mut 100_000, &|| false).unwrap().unwrap();
            for (actual, expected) in result.into_iter().zip([1., 1. / 6., 0., 1. / 3., 0.]) {
                assert!(
                    (actual - expected).abs() < 1e-15,
                    "actual={actual} exact={expected}"
                );
            }
        }
        assert_eq!(point_key(Point::new(-0., 0.)), Some([0, 0]));
        assert!(point_key(Point::new(f64::NAN, 0.)).is_none());
        assert!(point_key(Point::new(-f64::from_bits(1), 0.)).is_none());
    }

    #[test]
    fn area_hint_never_authorizes_holes_overlaps_or_t_junctions() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        for split in [0.5f64.next_up(), 0.5f64.next_down()] {
            let mut inputs = rectangle(0., 0., 0.5, 1.);
            inputs.extend(rectangle(split, 0., 1., 1.));
            assert_eq!(
                run(&inputs, &admit, &mut 100_000, &|| false).unwrap(),
                None,
                "split={split}"
            );
        }
        let mut duplicate = rectangle(0., 0., 0.5, 1.);
        duplicate.extend(rectangle(0., 0., 0.5, 1.));
        assert_eq!(run(&duplicate, &admit, &mut 100_000, &|| false).unwrap(), None);
        let mut junction = rectangle(0., 0., 0.5, 1.);
        junction.extend(rectangle(0.5, 0., 1., 0.5));
        junction.extend(rectangle(0.5, 0.5, 1., 1.));
        assert_eq!(run(&junction, &admit, &mut 100_000, &|| false).unwrap(), None);
        assert_eq!(
            run(&rectangle(0., 0., 0.5, 1.), &admit, &mut 100_000, &|| false).unwrap(),
            None
        );
    }

    #[test]
    fn partial_nonoverlap_preserves_area_and_color_without_filling_gaps() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let evaluate = |input: &[Input]| {
            integrate_disjoint_unit_mesh(input.iter().copied().map(Ok), &admit, &mut 100_000, &|| false)
                .unwrap()
        };
        let partial = evaluate(&rectangle(0., 0., 0.5, 1.)).unwrap();
        for (actual, expected) in partial.into_iter().zip([0.5, 1. / 24., 0., 1. / 6., 0.]) {
            assert!((actual - expected).abs() < 1e-15);
        }
        let split = 0.5f64.next_up();
        let mut gap = rectangle(0., 0., 0.5, 1.);
        gap.extend(rectangle(split, 0., 1., 1.));
        assert_eq!(evaluate(&gap).unwrap()[0], 1. - (split - 0.5));
        let mut horizontal = rectangle(0., 0., 1., 0.25);
        horizontal.extend(rectangle(0., 0.75, 1., 1.));
        assert_eq!(evaluate(&horizontal).unwrap()[0], 0.5);
        let mut overlap = rectangle(0., 0., 0.6, 1.);
        overlap.extend(rectangle(0.4, 0., 0.8, 1.));
        assert_eq!(evaluate(&overlap), None);
        let mut duplicate = rectangle(0., 0., 0.5, 1.);
        duplicate.extend(rectangle(0., 0., 0.5, 1.));
        assert_eq!(evaluate(&duplicate), None);
        let mut junction = rectangle(0., 0., 0.5, 1.);
        junction.extend(rectangle(0.5, 0., 1., 0.5));
        junction.extend(rectangle(0.5, 0.5, 1., 1.));
        assert_eq!(evaluate(&junction).unwrap()[0], 1.);
    }

    #[test]
    fn grid_pair_certificate_never_accepts_positive_overlap() {
        let admit = |_: usize| Some(Box::new(()) as Credit);
        let mut boxes = Vec::new();
        for x0 in 0..4 {
            for x1 in x0 + 1..=4 {
                for y0 in 0..4 {
                    for y1 in y0 + 1..=4 {
                        boxes.push([x0, y0, x1, y1]);
                    }
                }
            }
        }
        let mut accepted = 0;
        for a in &boxes {
            for b in &boxes {
                let overlap = a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3]);
                let mut input = Vec::new();
                for bounds in [a, b] {
                    let [x0, y0, x1, y1] = bounds.map(|v| f64::from(v) / 4.);
                    input.extend(rectangle(x0, y0, x1, y1));
                }
                if let Some(value) =
                    integrate_disjoint_unit_mesh(input.iter().copied().map(Ok), &admit, &mut 100_000, &|| {
                        false
                    })
                    .unwrap()
                {
                    assert!(!overlap, "accepted overlapping boxes {a:?} {b:?}");
                    let area = |r: &[i32; 4]| f64::from((r[2] - r[0]) * (r[3] - r[1])) / 16.;
                    assert_eq!(value[0], area(a) + area(b));
                    accepted += 1;
                }
            }
        }
        assert!(accepted > 1000, "proof must accept useful disjoint cases");
        eprintln!("CELL grid_pairs=10000 accepted={accepted}; no positive overlap accepted");
    }

    struct Reservation(Arc<AtomicUsize>, usize);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, Ordering::SeqCst);
        }
    }
    #[test]
    fn every_admission_cancel_and_short_quota_releases_scratch_and_retries() {
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let deny = Cell::new(false);
        let admit = |bytes| {
            calls.set(calls.get() + 1);
            if deny.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::SeqCst);
            Some(Box::new(Reservation(used.clone(), bytes)) as Credit)
        };
        let mut horizontal = rectangle(0., 0., 1., 0.25);
        horizontal.extend(rectangle(0., 0.75, 1., 1.));
        for (name, input, require_full) in [
            ("full", rectangle(0., 0., 1., 1.), true),
            ("partial-x", rectangle(0., 0., 0.5, 1.), false),
            ("partial-y", horizontal, false),
        ] {
            calls.set(0);
            let run =
                |input: &[Input], admit: &Admitter<'_>, work: &mut usize, cancelled: &dyn Fn() -> bool| {
                    integrate_mesh(
                        input.iter().copied().map(Ok),
                        admit,
                        work,
                        cancelled,
                        require_full,
                    )
                };
            let mut work = 100_000;
            let checks = Cell::new(0);
            let expected = run(&input, &admit, &mut work, &|| {
                checks.set(checks.get() + 1);
                false
            })
            .unwrap();
            assert!(expected.is_some());
            assert_eq!(used.load(Ordering::SeqCst), 0);
            assert_eq!(calls.get(), 1);
            let cost = 100_000 - work;
            deny.set(true);
            assert_eq!(
                run(&input, &admit, &mut 100_000, &|| false),
                Err(PixelPartitionError::Admission)
            );
            assert_eq!(used.load(Ordering::SeqCst), 0);
            deny.set(false);
            for stop in 0..checks.get() {
                let at = Cell::new(0);
                assert_eq!(
                    run(&input, &admit, &mut 100_000, &|| {
                        let now = at.get();
                        at.set(now + 1);
                        now == stop
                    }),
                    Err(PixelPartitionError::Cancelled)
                );
                assert_eq!(used.load(Ordering::SeqCst), 0);
            }
            for mut quota in 0..cost {
                assert_eq!(
                    run(&input, &admit, &mut quota, &|| false),
                    Err(PixelPartitionError::Limit)
                );
                assert_eq!(used.load(Ordering::SeqCst), 0);
            }
            assert_eq!(
                run(&input, &admit, &mut cost.clone(), &|| false).unwrap(),
                expected
            );
            assert_eq!(used.load(Ordering::SeqCst), 0);
            eprintln!(
                "CELL mode={name} failures admission=1 cancellations={} short_quotas={cost}; retry exact; final=0",
                checks.get()
            );
        }
    }

    #[test]
    fn captured_mesh_matches_exact_affine_oracle_and_rejects_overlap_controls() {
        type Row = (usize, usize, usize, [[u64; 2]; 3], [[u64; 3]; 6]);
        let (dense, _): (&[Row], usize) =
            include!("../../../../../tests/fixtures/geometry/dense-cell-176.rs");
        let (overlap, _): (&[Row], usize) =
            include!("../../../../../tests/fixtures/geometry/coverage-cell-82.rs");
        let (gap, _): (&[Row], usize) =
            include!("../../../../../tests/fixtures/geometry/coverage-cell-103.rs");
        let admit = |_: usize| Some(Box::new(()) as Credit);
        for (name, rows, full) in [
            ("dense1236", dense, true),
            ("overlap116", overlap, false),
            ("gap328", gap, false),
        ] {
            let inputs = rows
                .iter()
                .map(|&(patch_index, sample_index, component_count, points, fields)| {
                    let f = fields.map(|v| v.map(f64::from_bits));
                    triangle(
                        points.map(|p| p.map(f64::from_bits)),
                        PixelSourceOwner {
                            patch_index,
                            sample_index,
                            component_count,
                            u: f[0],
                            v: f[1],
                            components: [f[2], f[3], f[4], f[5]],
                        },
                    )
                })
                .collect::<Vec<_>>();
            let mut work = 100_000_000;
            let result = run(&inputs, &admit, &mut work, &|| false).unwrap();
            assert_eq!(result.is_some(), full);
            let partial = integrate_disjoint_unit_mesh(
                inputs.iter().copied().map(Ok),
                &admit,
                &mut 100_000_000,
                &|| false,
            )
            .unwrap();
            assert_eq!(partial, result, "production proof disagrees on {name}");
            if let Some(integral) = result {
                // Exact Fraction affine integrals of the captured binary64
                // inputs from verify-ordered-full-cell.py, not curved-source
                // or output-color-space certificates.
                for (actual, expected) in integral.into_iter().zip([
                    1.,
                    0.2405217886322456,
                    0.739670798603927,
                    0.7757544471153687,
                    0.23368041473727055,
                ]) {
                    assert!(
                        (actual - expected).abs() < 2e-15,
                        "actual={actual} control={expected}"
                    );
                }
            }
            eprintln!(
                "FULL_CELL name={name} certified={} integral={result:?} work={}",
                result.is_some(),
                100_000_000 - work
            );
        }
    }
}
