//! Bounded fill-curve subdivision in transformed pixel space.
use kurbo::{Affine, BezPath, PathEl, Point};

/// Flatten fill curves with a control-hull bound on distance to each output
/// segment in transformed space. Refuses nonfinite inputs, depth above twenty,
/// or output beyond `max_elements`; it never returns a partial path.
pub fn flatten_fill_bounded(path: &BezPath, transform: Affine, tolerance: f64, max_elements: usize) -> Result<BezPath, ()> {
    flatten_fill_bounded_with_admission(path, transform, tolerance, max_elements, |_| Ok(())).map(|(path, ())| path)
}

/// Count output without allocation, admit exact path storage, then materialize.
/// The returned guard remains caller-owned for the complete path-consumption lifetime.
pub fn flatten_fill_bounded_with_admission<G>(path: &BezPath, transform: Affine, tolerance: f64, max_elements: usize, admit: impl FnOnce(usize) -> Result<G, ()>) -> Result<(BezPath, G), ()> {
    flatten_fill_bounded_with_admission_and_cancel(path, transform, tolerance, max_elements, admit, &|| false)
}

/// Poll cancellation at every subdivision node in both passes and before admission.
pub fn flatten_fill_bounded_with_admission_and_cancel<G>(path: &BezPath, transform: Affine, tolerance: f64, max_elements: usize, admit: impl FnOnce(usize) -> Result<G, ()>, cancelled: &dyn Fn() -> bool) -> Result<(BezPath, G), ()> {
    if cancelled() { return Err(()); }
    if !tolerance.is_finite() || tolerance <= 0.0 || max_elements == 0 || max_elements > 65536 || transform.as_coeffs().iter().any(|v| !v.is_finite()) { return Err(()); }
    let mut plan = Sink { count: 0, cap: max_elements, output: None };
    walk(path,transform,tolerance,&mut plan,cancelled)?;
    let bytes = plan.count.checked_mul(size_of::<PathEl>()).ok_or(())?;
    if cancelled() { return Err(()); }
    let guard = admit(bytes)?;
    if cancelled() { return Err(()); }
    let mut storage = Vec::new();
    storage.try_reserve_exact(plan.count).map_err(|_| ())?;
    let mut out = Sink { count: 0, cap: plan.count, output: Some(storage) };
    walk(path,transform,tolerance,&mut out,cancelled)?;
    Ok((BezPath::from_vec(out.output.unwrap()),guard))
}

struct Sink { count: usize, cap: usize, output: Option<Vec<PathEl>> }
impl Sink {
    fn push(&mut self, element: PathEl) -> Result<(), ()> {
        if self.count >= self.cap { return Err(()); }
        self.count += 1;
        if let Some(output) = &mut self.output { output.push(element); }
        Ok(())
    }
}
fn walk(path: &BezPath, transform: Affine, tolerance: f64, out: &mut Sink, cancelled: &dyn Fn() -> bool) -> Result<(), ()> {
    let mut current = Point::ZERO;
    let mut start = Point::ZERO;
    fn distance(point: Point, a: Point, b: Point) -> f64 {
        let v = b-a;
        let length = v.hypot2();
        let t = if length > 0.0 { ((point-a).dot(v)/length).clamp(0.0,1.0) } else { 0.0 };
        (point-(a+v*t)).hypot2()
    }
    fn curve(p: [Point;4], tf: Affine, tol2: f64, depth: u8, out: &mut Sink, cancelled: &dyn Fn() -> bool) -> Result<(), ()> {
        if cancelled() { return Err(()); }
        let q = p.map(|point| tf*point);
        if q.iter().any(|point| !point.x.is_finite() || !point.y.is_finite()) { return Err(()); }
        if distance(q[1],q[0],q[3]).max(distance(q[2],q[0],q[3])) <= tol2 {
            return out.push(PathEl::LineTo(p[3]));
        }
        if depth == 20 { return Err(()); }
        let a=p[0].midpoint(p[1]);let b=p[1].midpoint(p[2]);let c=p[2].midpoint(p[3]);
        let d=a.midpoint(b);let e=b.midpoint(c);let f=d.midpoint(e);
        curve([p[0],a,d,f],tf,tol2,depth+1,out,cancelled)?;
        curve([f,e,c,p[3]],tf,tol2,depth+1,out,cancelled)
    }
    for element in path.elements() {
        if cancelled() { return Err(()); }
        match *element {
            PathEl::MoveTo(p) => { if !p.x.is_finite() || !p.y.is_finite() { return Err(()); } current=p;start=p;out.push(*element)?; }
            PathEl::LineTo(p) => { if !p.x.is_finite() || !p.y.is_finite() { return Err(()); } current=p;out.push(*element)?; }
            PathEl::QuadTo(p,end) => { let a=current+(p-current)*(2.0/3.0);let b=end+(p-end)*(2.0/3.0);curve([current,a,b,end],transform,tolerance*tolerance,0,out,cancelled)?;current=end; }
            PathEl::CurveTo(a,b,end) => { curve([current,a,b,end],transform,tolerance*tolerance,0,out,cancelled)?;current=end; }
            PathEl::ClosePath => { current=start;out.push(*element)?; }
        }
    }
    Ok(())
}
