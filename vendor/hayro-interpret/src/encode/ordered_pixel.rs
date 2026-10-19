//! Admitted local-pixel ownership partition for affine source fields.
//! Used by the sampled Tensor/Coons patch path. Floating-point clipping
//! is numerical geometry, not an interval certificate for the original surface.
use kurbo::Point;

type Credit = Box<dyn std::any::Any + Send + Sync>;
type Admitter<'a> = dyn Fn(usize) -> Option<Credit> + 'a;
const VERTICES: usize = 16;

/// Explicit failure leaves the previously published partition unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelPartitionError {
    /// Invalid/nonfinite source data or geometry.
    Invalid,
    /// Source-to-pixel affine inversion cannot be resolved reliably.
    Conditioning,
    /// Managed allocation or fallible allocation refused.
    Admission,
    /// Configured fragment, vertex or operation bound exceeded.
    Limit,
    /// The caller requested cancellation.
    Cancelled,
}
type Result<T> = std::result::Result<T, PixelPartitionError>;

/// Affine fields are `[constant, local_x, local_y]` in the unit pixel.
/// The caller must qualify these fields against its source approximation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelSourceOwner {
    /// Later source patches cover earlier patches.
    pub patch_index: usize,
    /// Caller sampling identifier, retained with fragments; never affects priority.
    pub sample_index: usize,
    /// Original u in the source patch.
    pub u: [f64; 3],
    /// Original v; folded priority is greatest v, then greatest u.
    pub v: [f64; 3],
    /// Original source components, before any nonlinear function/conversion.
    pub components: [[f64; 3]; 4],
    /// Active source component count, from one to four.
    pub component_count: usize,
}
impl PixelSourceOwner {
    /// Convert one provenance-carrying device-space triangle to local fields.
    /// Uses a compensated determinant and checks vertex reconstruction. This
    /// qualifies the affine triangle only, not its original curved surface or
    /// equality of parameter fields from different triangles. Singular and
    /// poorly conditioned input is refused rather than assigned guessed priority.
    /// No allocation is performed.
    pub fn from_triangle(
        triangle: &crate::shading::Triangle,
        source: crate::shading::PatchTriangleSource,
        pixel_origin: Point,
    ) -> Result<Self> {
        let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
        let count = vertices[0].colors.len();
        if !(1..=4).contains(&count)
            || !pixel_origin.x.is_finite()
            || !pixel_origin.y.is_finite()
            || vertices.iter().any(|v| {
                v.colors.len() != count
                    || !v.point.x.is_finite()
                    || !v.point.y.is_finite()
                    || v.colors.iter().any(|c| !c.is_finite())
            })
            || source
                .parameters
                .iter()
                .any(|p| !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
        {
            return Err(PixelPartitionError::Invalid);
        }
        let points = vertices.map(|v| Point::new(v.point.x - pixel_origin.x, v.point.y - pixel_origin.y));
        if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(PixelPartitionError::Conditioning);
        }
        let a = points[1] - points[0];
        let b = points[2] - points[0];
        fn difference(a: f64, b: f64, c: f64, d: f64) -> f64 {
            let product = c * d;
            a.mul_add(b, -product) - c.mul_add(d, -product)
        }
        let determinant = difference(a.x, b.y, a.y, b.x);
        let scale = (a.x * b.y).abs() + (a.y * b.x).abs();
        if !determinant.is_finite() || !scale.is_finite() || determinant.abs() <= 128. * f64::EPSILON * scale
        {
            return Err(PixelPartitionError::Conditioning);
        }
        let fit = |values: [f64; 3]| -> Result<[f64; 3]> {
            if values.iter().all(|v| *v == values[0]) {
                return Ok([values[0], 0., 0.]);
            }
            let q = values[1] - values[0];
            let r = values[2] - values[0];
            let x = difference(q, b.y, r, a.y) / determinant;
            let y = difference(a.x, r, b.x, q) / determinant;
            let constant = (-x).mul_add(points[0].x, (-y).mul_add(points[0].y, values[0]));
            let field = [constant, x, y];
            if field.iter().any(|v| !v.is_finite()) {
                return Err(PixelPartitionError::Conditioning);
            }
            let input_magnitude = values.iter().map(|v| v.abs()).fold(0., f64::max);
            for (point, expected) in points.into_iter().zip(values) {
                let actual = value(field, point);
                let bound = 64.
                    * f64::EPSILON
                    * (constant.abs()
                        + (x * point.x).abs()
                        + (y * point.y).abs()
                        + expected.abs()
                        + input_magnitude);
                if !actual.is_finite() || !bound.is_finite() || (actual - expected).abs() > bound {
                    return Err(PixelPartitionError::Conditioning);
                }
            }
            Ok(field)
        };
        let mut components = [[0.; 3]; 4];
        for i in 0..count {
            components[i] = fit(vertices.map(|v| f64::from(v.colors[i])))?;
        }
        Ok(Self {
            patch_index: source.patch_index,
            sample_index: 0,
            u: fit(source.parameters.map(|p| p.x))?,
            v: fit(source.parameters.map(|p| p.y))?,
            components,
            component_count: count,
        })
    }
    fn valid(&self) -> bool {
        (1..=4).contains(&self.component_count)
            && self.u.iter().chain(&self.v).all(|v| v.is_finite())
            && self.components.iter().flatten().all(|v| v.is_finite())
    }
    /// Evaluate the retained source fields at a local point. Integration of a
    /// nonlinear source function must happen over fragments, not after averaging.
    pub fn evaluate(&self, point: Point) -> Option<[f64; 4]> {
        let values = self.components.map(|c| value(c, point));
        values.iter().all(|v| v.is_finite()).then_some(values)
    }
}
fn value(c: [f64; 3], p: Point) -> f64 {
    c[1].mul_add(p.x, c[2].mul_add(p.y, c[0]))
}
fn tick(work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        return Err(PixelPartitionError::Cancelled);
    }
    *work = work.checked_sub(1).ok_or(PixelPartitionError::Limit)?;
    Ok(())
}

// Exact sign for represented finite coordinates. Fast ordinary determinants
// use a conservative filter; ambiguous/overflowing/subnormal cases sum six
// dyadic products in fixed storage (no allocator or geometric epsilon).
fn orientation(a: Point, b: Point, c: Point) -> i8 {
    if a == b || b == c || a == c || (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y) {
        return 0;
    }
    let u = b - a;
    let v = c - a;
    let left = u.x * v.y;
    let right = u.y * v.x;
    let det = left - right;
    let bound = 64. * f64::EPSILON * (left.abs() + right.abs()) + 64. * f64::from_bits(1);
    if det.is_finite() && bound.is_finite() && det.abs() > bound {
        return if det > 0. { 1 } else { -1 };
    }
    fn parts(value: f64) -> (u64, i32, bool) {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 2047) as i32;
        let mantissa = (bits & ((1u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1u64 << 52 };
        (
            mantissa,
            if exponent == 0 {
                -1074
            } else {
                exponent - 1023 - 52
            },
            bits >> 63 != 0,
        )
    }
    fn add(words: &mut [u64; 68], mut at: usize, mut value: u64) {
        while value != 0 {
            let (next, carry) = words[at].overflowing_add(value);
            words[at] = next;
            value = u64::from(carry);
            at += 1;
        }
    }
    let mut positive = [0u64; 68];
    let mut negative = [0u64; 68];
    for (x, y, subtract) in [
        (a.x, b.y, false),
        (b.x, c.y, false),
        (c.x, a.y, false),
        (a.y, b.x, true),
        (b.y, c.x, true),
        (c.y, a.x, true),
    ] {
        let (xm, xe, xs) = parts(x);
        let (ym, ye, ys) = parts(y);
        let product = u128::from(xm) * u128::from(ym);
        if product == 0 {
            continue;
        }
        let shift = (xe + ye + 2148) as usize;
        let at = shift / 64;
        let bits = shift % 64;
        let lo = product as u64;
        let hi = (product >> 64) as u64;
        let words = if xs ^ ys ^ subtract {
            &mut negative
        } else {
            &mut positive
        };
        if bits == 0 {
            add(words, at, lo);
            add(words, at + 1, hi);
        } else {
            add(words, at, lo << bits);
            add(words, at + 1, (hi << bits) | (lo >> (64 - bits)));
            add(words, at + 2, hi >> (64 - bits));
        }
    }
    for i in (0..68).rev() {
        match positive[i].cmp(&negative[i]) {
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Equal => {}
        }
    }
    0
}

#[derive(Clone, Copy)]
struct Polygon {
    points: [Point; VERTICES],
    len: usize,
}
impl Polygon {
    fn empty() -> Self {
        Self {
            points: [Point::ZERO; VERTICES],
            len: 0,
        }
    }
    fn push(&mut self, point: Point) -> Result<()> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(PixelPartitionError::Invalid);
        }
        if self.len > 0 && self.points[self.len - 1] == point {
            return Ok(());
        }
        if self.len == VERTICES {
            return Err(PixelPartitionError::Limit);
        }
        self.points[self.len] = point;
        self.len += 1;
        Ok(())
    }
    fn convex_hull(mut self) -> Self {
        // Clipped vertices are rounded doubles. Restore the convex invariant
        // without an epsilon: tiny reflex/collinear runs otherwise turn short
        // edges into invalid clipping half-planes on subsequent overlays.
        if self.len < 3 {
            return self;
        }
        self.points[..self.len].sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap()
                .then(a.y.partial_cmp(&b.y).unwrap())
        });
        let mut unique = 0;
        for i in 0..self.len {
            if unique == 0 || self.points[i] != self.points[unique - 1] {
                self.points[unique] = self.points[i];
                unique += 1;
            }
        }
        if unique < 3 {
            self.len = unique;
            return self;
        }
        let mut hull = [Point::ZERO; VERTICES * 2];
        let mut n = 0;
        let turn = orientation;
        for point in self.points[..unique].iter().copied() {
            while n >= 2 && turn(hull[n - 2], hull[n - 1], point) <= 0 {
                n -= 1;
            }
            hull[n] = point;
            n += 1;
        }
        let lower = n;
        for point in self.points[..unique - 1].iter().copied().rev() {
            while n > lower && turn(hull[n - 2], hull[n - 1], point) <= 0 {
                n -= 1;
            }
            hull[n] = point;
            n += 1;
        }
        n -= 1;
        debug_assert!(n <= unique);
        self.points[..n].copy_from_slice(&hull[..n]);
        self.len = n;
        self
    }
    fn moments(&self) -> Option<(f64, Point)> {
        if self.len < 3 {
            return None;
        }
        let mut area = 0.;
        let mut x = 0.;
        let mut y = 0.;
        // Anchor the fan at a polygon vertex: thin fragments near a pixel
        // edge must not subtract nearly equal world/local-origin cross sums.
        let origin = self.points[0];
        for i in 1..self.len - 1 {
            let a = self.points[i] - origin;
            let b = self.points[i + 1] - origin;
            let product = a.y * b.x;
            let c = a.x.mul_add(b.y, -product) - a.y.mul_add(b.x, -product);
            area += c;
            x += (a.x + b.x) * c;
            y += (a.y + b.y) * c;
        }
        if area == 0. {
            return None;
        }
        let centroid = Point::new(origin.x + x / (3. * area), origin.y + y / (3. * area));
        if !area.is_finite() || !centroid.x.is_finite() || !centroid.y.is_finite() {
            return None;
        }
        Some((area.abs() * 0.5, centroid))
    }
    fn clip(&self, line: [f64; 3], work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<Self> {
        self.clip_by(|p| value(line, p), work, cancelled)
    }
    fn clip_by(
        &self,
        evaluate: impl Fn(Point) -> f64,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self> {
        tick(work, cancelled)?;
        let mut out = Self::empty();
        if self.len == 0 {
            return Ok(out);
        }
        let mut previous = self.points[self.len - 1];
        let mut a = evaluate(previous);
        if !a.is_finite() {
            return Err(PixelPartitionError::Invalid);
        }
        for current in self.points[..self.len].iter().copied() {
            tick(work, cancelled)?;
            let b = evaluate(current);
            if !b.is_finite() {
                return Err(PixelPartitionError::Invalid);
            }
            if (a >= 0.) != (b >= 0.) {
                let denominator = a - b;
                if !denominator.is_finite() {
                    return Err(PixelPartitionError::Invalid);
                }
                let t = a / denominator;
                if !t.is_finite() {
                    return Err(PixelPartitionError::Invalid);
                }
                out.push(previous + (current - previous) * t)?;
            }
            if b >= 0. {
                out.push(current)?;
            }
            previous = current;
            a = b;
        }
        if out.len > 1 && out.points[0] == out.points[out.len - 1] {
            out.len -= 1;
        }
        Ok(out.convex_hull())
    }
    // Join only identical-owner convex pieces sharing an exact reversed edge.
    // Never replace a concave union by its hull or merge across sampling IDs.
    fn convex_union(&self, other: &Self) -> Option<Self> {
        if self.len + other.len - 2 > VERTICES {
            return None;
        }
        for i in 0..self.len {
            for j in 0..other.len {
                if self.points[i] != other.points[(j + 1) % other.len]
                    || self.points[(i + 1) % self.len] != other.points[j]
                {
                    continue;
                }
                let mut joined = Self::empty();
                for k in 0..self.len {
                    joined.push(self.points[(i + 1 + k) % self.len]).ok()?;
                }
                for k in 1..other.len - 1 {
                    joined.push(other.points[(j + 1 + k) % other.len]).ok()?;
                }
                for k in 0..joined.len {
                    let a = joined.points[(k + 1) % joined.len] - joined.points[k];
                    let b = joined.points[(k + 2) % joined.len] - joined.points[(k + 1) % joined.len];
                    let product = a.y * b.x;
                    let cross = a.x.mul_add(b.y, -product) - a.y.mul_add(b.x, -product);
                    let scale = (a.x * b.y).abs() + (a.y * b.x).abs();
                    let roundoff = 32. * f64::EPSILON * scale + 32. * f64::from_bits(1);
                    // Optional merging must not classify a numerically ambiguous
                    // turn. Retain both original pieces in that case.
                    if !cross.is_finite() || !roundoff.is_finite() || cross <= roundoff {
                        return None;
                    }
                }
                let first = (0..joined.len).min_by(|&i, &j| {
                    joined.points[i]
                        .x
                        .total_cmp(&joined.points[j].x)
                        .then(joined.points[i].y.total_cmp(&joined.points[j].y))
                })?;
                joined.points[..joined.len].rotate_left(first);
                return Some(joined);
            }
        }
        None
    }

    fn bounds_overlap(&self, other: &Self) -> bool {
        let bounds = |polygon: &Self| {
            let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
            for p in &polygon.points[..polygon.len] {
                b[0] = b[0].min(p.x);
                b[1] = b[1].min(p.y);
                b[2] = b[2].max(p.x);
                b[3] = b[3].max(p.y);
            }
            b
        };
        let a = bounds(self);
        let b = bounds(other);
        a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
    }
    fn edges(&self) -> impl Iterator<Item = [Point; 2]> + '_ {
        (0..self.len).map(|i| [self.points[i], self.points[(i + 1) % self.len]])
    }
}
#[derive(Clone, Copy)]
struct Fragment {
    polygon: Polygon,
    owner: PixelSourceOwner,
}
struct AdmittedVec<'a, T> {
    values: Vec<T>,
    credit: Option<Credit>,
    admit: &'a Admitter<'a>,
    maximum: usize,
}
impl<'a, T> AdmittedVec<'a, T> {
    fn new(admit: &'a Admitter<'a>, maximum: usize) -> Self {
        Self {
            values: Vec::new(),
            credit: None,
            admit,
            maximum,
        }
    }
    fn push(&mut self, item: T) -> Result<()> {
        if self.values.len() == self.maximum {
            return Err(PixelPartitionError::Limit);
        }
        if self.values.len() == self.values.capacity() {
            let capacity = self
                .values
                .capacity()
                .checked_mul(2)
                .ok_or(PixelPartitionError::Limit)?
                .max(4)
                .min(self.maximum);
            let bytes = capacity
                .checked_mul(size_of::<T>())
                .ok_or(PixelPartitionError::Limit)?;
            // Admit replacement while old credit/allocation remain live.
            let next = (self.admit)(bytes).ok_or(PixelPartitionError::Admission)?;
            self.values
                .try_reserve_exact(capacity - self.values.len())
                .map_err(|_| PixelPartitionError::Admission)?;
            if self.values.capacity() > capacity {
                return Err(PixelPartitionError::Admission);
            }
            self.credit = Some(next);
        }
        self.values.push(item);
        Ok(())
    }
}
fn keep(out: &mut AdmittedVec<'_, Fragment>, polygon: Polygon, owner: PixelSourceOwner) -> Result<()> {
    if polygon.moments().is_some() {
        if let Some(previous) = out.values.last_mut() {
            if previous.owner == owner {
                if let Some(joined) = previous.polygon.convex_union(&polygon) {
                    previous.polygon = joined;
                    return Ok(());
                }
            }
        }
        out.push(Fragment { polygon, owner })?;
    }
    Ok(())
}
// Emit convex pieces of `polygon \ cut`, returning the remaining intersection.
fn subtract(
    polygon: Polygon,
    cut: &Polygon,
    owner: PixelSourceOwner,
    out: &mut AdmittedVec<'_, Fragment>,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Polygon> {
    tick(work, cancelled)?;
    if !polygon.bounds_overlap(cut) {
        keep(out, polygon, owner)?;
        return Ok(Polygon::empty());
    }
    let mut inside = polygon;
    for [a, b] in cut.edges() {
        tick(work, cancelled)?;
        // Local anchor avoids cancellation in ax*by-bx*ay for very short
        // edges near a unit-pixel boundary. Preserve orientation with FMA.
        let edge = |p: Point| {
            let d = b - a;
            let q = p - a;
            let product = d.y * q.x;
            d.x.mul_add(q.y, -product) - d.y.mul_add(q.x, -product)
        };
        keep(out, inside.clip_by(|p| -edge(p), work, cancelled)?, owner)?;
        inside = inside.clip_by(edge, work, cancelled)?;
        if inside.moments().is_none() {
            break;
        }
    }
    Ok(inside)
}
/// An admitted partition of one unit pixel into disjoint convex source regions.
/// Painting is transactional. Limits apply to retained and temporary fragment
/// vectors independently; a shared operation budget bounds all clipping work.
pub struct OrderedPixel<'a> {
    regions: AdmittedVec<'a, Fragment>,
}
impl<'a> OrderedPixel<'a> {
    /// No allocation occurs until the first nonempty triangle is painted.
    pub fn new(admit: &'a Admitter<'a>, maximum_fragments: usize) -> Self {
        Self {
            regions: AdmittedVec::new(admit, maximum_fragments),
        }
    }
    /// Retained disjoint region count, not triangle multiplicity.
    pub fn fragments(&self) -> usize {
        self.regions.values.len()
    }
    /// Visit geometric fragments with area/centroid and unaveraged owner fields.
    pub fn visit(
        &self,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
        visitor: &mut dyn FnMut(f64, Point, &[Point], &PixelSourceOwner) -> Result<()>,
    ) -> Result<()> {
        for fragment in &self.regions.values {
            tick(work, cancelled)?;
            let (area, point) = fragment.polygon.moments().ok_or(PixelPartitionError::Invalid)?;
            visitor(
                area,
                point,
                &fragment.polygon.points[..fragment.polygon.len],
                &fragment.owner,
            )?;
        }
        Ok(())
    }
    /// Intersect retained regions with a convex local clip (three or four
    /// cyclic vertices), without heap scratch. Both orientations are accepted;
    /// nonconvex/self-crossing clips refuse. Visitor work shares the same budget
    /// and may not increase it. Discard accumulated results on any error.
    pub fn visit_convex_clip(
        &self,
        points: &[Point],
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
        visitor: &mut dyn FnMut(f64, Point, &[Point], &PixelSourceOwner, &mut usize) -> Result<()>,
    ) -> Result<()> {
        tick(work, cancelled)?;
        if !(3..=4).contains(&points.len()) {
            return Err(PixelPartitionError::Invalid);
        }
        let mut clip = Polygon::empty();
        for &point in points {
            tick(work, cancelled)?;
            clip.push(point)?;
        }
        let mut orientation = 0.;
        for i in 0..clip.len {
            tick(work, cancelled)?;
            let a = clip.points[(i + 1) % clip.len] - clip.points[i];
            let b = clip.points[(i + 2) % clip.len] - clip.points[(i + 1) % clip.len];
            let product = a.y * b.x;
            let cross = a.x.mul_add(b.y, -product) - a.y.mul_add(b.x, -product);
            if !cross.is_finite() {
                return Err(PixelPartitionError::Invalid);
            }
            if cross != 0. {
                if orientation != 0. && cross.signum() != orientation {
                    return Err(PixelPartitionError::Invalid);
                }
                orientation = cross.signum();
            }
        }
        if orientation == 0. {
            return Ok(());
        }
        if orientation < 0. {
            clip.points[..clip.len].reverse();
        }
        clip = clip.convex_hull();
        for fragment in &self.regions.values {
            tick(work, cancelled)?;
            if !fragment.polygon.bounds_overlap(&clip) {
                continue;
            }
            let mut polygon = fragment.polygon;
            for [a, b] in clip.edges() {
                let edge = |p: Point| {
                    let d = b - a;
                    let q = p - a;
                    let product = d.y * q.x;
                    d.x.mul_add(q.y, -product) - d.y.mul_add(q.x, -product)
                };
                polygon = polygon.clip_by(edge, work, cancelled)?;
                if polygon.moments().is_none() {
                    break;
                }
            }
            if let Some((area, center)) = polygon.moments() {
                let before = *work;
                let result = visitor(
                    area,
                    center,
                    &polygon.points[..polygon.len],
                    &fragment.owner,
                    work,
                );
                if *work > before {
                    *work = before;
                    return Err(PixelPartitionError::Invalid);
                }
                result?;
                tick(work, cancelled)?;
            }
        }
        Ok(())
    }

    /// Apply an opaque source triangle in local pixel coordinates. Ownership is
    /// independent of submission order: later patch, then greatest v, then u.
    /// Equality of v fields is exact equality of supplied affine coefficients;
    /// conditioning/certification against the original surface belongs to caller.
    pub fn paint_triangle(
        &mut self,
        mut points: [Point; 3],
        owner: PixelSourceOwner,
        work: &mut usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        tick(work, cancelled)?;
        if !owner.valid() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(PixelPartitionError::Invalid);
        }
        let cross = (points[1] - points[0]).cross(points[2] - points[0]);
        if !cross.is_finite() {
            return Err(PixelPartitionError::Invalid);
        }
        if cross == 0. {
            return Ok(());
        }
        if cross < 0. {
            points.swap(1, 2);
        }
        let mut triangle = Polygon::empty();
        for p in points {
            triangle.push(p)?;
        }
        for line in [[0., 1., 0.], [1., -1., 0.], [0., 0., 1.], [1., 0., -1.]] {
            triangle = triangle.clip(line, work, cancelled)?;
        }
        if triangle.moments().is_none() {
            return Ok(());
        }
        let admit = self.regions.admit;
        let maximum = self.regions.maximum;
        let mut next = AdmittedVec::new(admit, maximum);
        let mut incoming = AdmittedVec::new(admit, maximum);
        incoming.push(Fragment {
            polygon: triangle,
            owner,
        })?;
        for old in &self.regions.values {
            tick(work, cancelled)?;
            let prefix = next.values.len();
            let previous_last = next.values.last().copied();
            let overlap = subtract(old.polygon, &triangle, old.owner, &mut next, work, cancelled)?;
            if overlap.moments().is_none() {
                continue;
            }
            let old_wins = if owner.patch_index > old.owner.patch_index {
                Polygon::empty()
            } else if owner.patch_index < old.owner.patch_index {
                overlap
            } else {
                let mut difference = std::array::from_fn::<_, 3, _>(|i| old.owner.v[i] - owner.v[i]);
                if difference.iter().all(|v| *v == 0.) {
                    difference = std::array::from_fn(|i| old.owner.u[i] - owner.u[i]);
                }
                if difference.iter().all(|v| *v == 0.) {
                    Polygon::empty()
                } else {
                    overlap.clip(difference, work, cancelled)?
                }
            };
            if old_wins.moments().is_none() {
                continue;
            }
            // A losing incoming triangle must not subdivide a wholly winning
            // retained region. Equality is of represented vertices, without a
            // tolerance or a guess about the original curved source.
            let whole_old_wins = old_wins.len == overlap.len
                && old_wins.points[..old_wins.len] == overlap.points[..overlap.len];
            if whole_old_wins {
                next.values.truncate(prefix);
                if let Some(previous) = previous_last {
                    next.values[prefix - 1] = previous;
                }
                keep(&mut next, old.polygon, old.owner)?;
            } else {
                keep(&mut next, old_wins, old.owner)?;
            }
            let mut remaining = AdmittedVec::new(admit, maximum);
            for part in &incoming.values {
                let cut = if whole_old_wins { &old.polygon } else { &old_wins };
                subtract(part.polygon, cut, owner, &mut remaining, work, cancelled)?;
            }
            incoming = remaining;
        }
        for part in incoming.values.iter().copied() {
            keep(&mut next, part.polygon, owner)?;
        }
        tick(work, cancelled)?;
        self.regions = next;
        Ok(())
    }
}

#[cfg(test)]
mod exact_orientation_tests {
    use super::*;
    #[test]
    fn filtered_orientation_matches_independent_fraction_oracle() {
        let cases = include!("../../../../tests/fixtures/geometry/orientation-sign-cases.rs");
        for (bits, expected) in cases {
            let v = bits.map(f64::from_bits);
            let [a, b, c] = [
                Point::new(v[0], v[1]),
                Point::new(v[2], v[3]),
                Point::new(v[4], v[5]),
            ];
            assert_eq!(orientation(a, b, c), expected, "{v:?}");
            assert_eq!(orientation(b, c, a), expected, "cyclic {v:?}");
            assert_eq!(orientation(a, c, b), -expected, "reverse {v:?}");
        }
    }
}
