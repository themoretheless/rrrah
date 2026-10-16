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
            let (a,b)=if self.a.y<self.b.y {(self.a,self.b)} else {(self.b,self.a)};
            (b.x-a.x)/(b.y-a.y)
        }
    }
    fn x(self, y: f64) -> f64 {
        // Shared edges use the same arithmetic anchor in either traversal
        // direction. Winding remains oriented independently below.
        let a=if self.a.y<=self.b.y {self.a} else {self.b};
        a.x + (y - a.y) * self.slope()
    }
    fn winding(self) -> i32 {
        if self.b.y > self.a.y { 1 } else { -1 }
    }
}
#[test]
fn shared_edge_interpolation_is_bit_identical_under_reverse_traversal() {
    for (a,b) in [(Point::new(0.1,0.2),Point::new(0.8,0.9)),
        (Point::new(-10000.25,-20000.125),Point::new(10000.75,20000.875)),
        (Point::new(-0.,-1.),Point::new(0.,1.)),
        (Point::new(0.25,0.5),Point::new(0.75,f64::from_bits(0.5f64.to_bits()+1)))] {
        let edge=Edge {a,b,region:0};let reversed=Edge {a:b,b:a,region:1};
        assert_eq!(edge.slope().to_bits(),reversed.slope().to_bits());
        assert_eq!(edge.winding(),-reversed.winding());
        for t in [0.,0.125,0.5,0.875,1.] {
            let y=a.y+(b.y-a.y)*t;
            assert_eq!(edge.x(y).to_bits(),reversed.x(y).to_bits());
        }
    }
}

/// One convex covered strip of a global pixel. For an affine component field,
/// its integral is exactly area times the value at the centroid, up to floating
/// point arithmetic. Arbitrary nonlinear fields need further integration.
#[derive(Clone,Copy,Debug)]
pub(super) struct PixelFragment {
    pub(super) vertices:[Point;4],
    pub(super) area:f64,
    pub(super) centroid:Point,
    /// Selected region index; zero for the ordinary all-region intersection.
    pub(super) owner:usize,
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
                self.left=0;
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
    row.pixel_area_with_work::<false>(regions, x, y, &mut work, admit, None, None, None, None,0,None)
}

pub(super) struct PolygonRow {
    right:Option<f64>,
    top:f64,
    bottom:f64,
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

/// Prepare source-owner regions: at most 4096 regions and 4096 row-active
/// edges. The ordinary intersection API retains its separate 64-region cap.
/// Storage and crossing work use the same admitted, bounded preparation.
pub(super) fn admitted_selected_polygon_row(
    regions:&[(&BezPath,Fill)],y:i32,work_limit:usize,cancelled:&dyn Fn()->bool,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
)->Option<PolygonRow> {
    if regions.is_empty() || regions.len()>4096 || y.abs_diff(0)>1_048_576 {return None;}
    let mut work=Work {left:work_limit,cancelled};
    prepare_row(regions,y,&mut work,admit)
}

/// Consume a shared allowance, including work spent before refusal.
pub(super) fn admitted_selected_polygon_row_budget(
    regions:&[(&BezPath,Fill)],y:i32,remaining:&mut usize,cancelled:&dyn Fn()->bool,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
)->Option<PolygonRow> {
    if regions.is_empty() || regions.len()>4096 || y.abs_diff(0)>1_048_576 {return None;}
    let mut work=Work {left:*remaining,cancelled};
    let result=prepare_row(regions,y,&mut work,admit);
    *remaining=work.left;result
}

/// Prepare only the requested vertical domain; retain original edge endpoints.
pub(super) fn admitted_selected_polygon_domain_budget(
    regions:&[(&BezPath,Fill)],y:i32,top:f64,bottom:f64,remaining:&mut usize,
    cancelled:&dyn Fn()->bool,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
)->Option<PolygonRow> {
    if regions.is_empty() || regions.len()>4096 || y.abs_diff(0)>1_048_576
        || !top.is_finite() || !bottom.is_finite() || top<f64::from(y)
        || bottom>f64::from(y)+1. || top>=bottom {return None;}
    let mut work=Work {left:*remaining,cancelled};
    let result=prepare_row_domain(regions,y,top,bottom,&mut work,admit);
    *remaining=work.left;result
}

fn prepare_row(
    regions: &[(&BezPath, Fill)],
    y: i32,
    work: &mut Work<'_>,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PolygonRow> {
    prepare_row_domain(regions,y,f64::from(y),f64::from(y)+1.,work,admit)
}

fn prepare_row_domain(
    regions:&[(&BezPath,Fill)],y:i32,top:f64,bottom:f64,work:&mut Work<'_>,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
)->Option<PolygonRow> {prepare_row_domain_right(regions,y,top,bottom,None,work,admit)}

/// Prepare geometry only for this pixel and subrectangles contained in it.
/// Continuous chains strictly left of the pixel become vertical winding edges;
/// all winding contributions and interior source edges remain represented.
/// Pixel storage allows max(4096, 3 * region_count) active edges (at most
/// 12288); allocation admission and shared crossing-work limits still apply.
pub(super) fn admitted_selected_polygon_pixel_row_budget(
    regions:&[(&BezPath,Fill)],x:i32,y:i32,remaining:&mut usize,cancelled:&dyn Fn()->bool,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
)->Option<PolygonRow> {
    if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576 || y.abs_diff(0)>1_048_576 {return None;}
    let mut work=Work {left:*remaining,cancelled};
    let result=prepare_row_domain_right(regions,y,f64::from(y),f64::from(y)+1.,Some(f64::from(x)+1.),&mut work,admit);
    *remaining=work.left;result
}

fn prepare_row_domain_right(
    regions: &[(&BezPath, Fill)],
    y: i32, top:f64,bottom:f64,right:Option<f64>,
    work: &mut Work<'_>,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Option<PolygonRow> {
    let cancelled = work.cancelled;
    let mut walk = |out: &mut dyn FnMut(Edge) -> Option<()>| -> Option<()> {
        for (region, (path, _)) in regions.iter().enumerate() {
            let mut current = None;
            let mut start = None;
            let mut left_chain = None;
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
                match element {
                    PathEl::MoveTo(p) => {
                        if let (Some(a), Some(b)) = (current, start) {
                            emit_row_edge(a,b,region,top,bottom,right,&mut left_chain,out)?;
                        }
                        flush_left_chain(&mut left_chain,region,top,bottom,right,out)?;
                        current = Some(p);
                        start = Some(p);
                    }
                    PathEl::LineTo(p) => {
                        emit_row_edge(current?,p,region,top,bottom,right,&mut left_chain,out)?;
                        current = Some(p);
                    }
                    PathEl::ClosePath => {
                        emit_row_edge(current?,start?,region,top,bottom,right,&mut left_chain,out)?;
                        current = start;
                    }
                    _ => unreachable!(),
                }
            }
            if let (Some(a), Some(b)) = (current, start) {
                if a != b {
                    emit_row_edge(a,b,region,top,bottom,right,&mut left_chain,out)?;
                }
            }
            flush_left_chain(&mut left_chain,region,top,bottom,right,out)?;
        }
        Some(())
    };
    // A pixel arrangement can contain three active edges per triangular
    // source-owner region. The general reusable row retains its old limit.
    // Memory is admitted below and all crossing work remains budgeted.
    let edge_limit=if right.is_some() {4096usize.max(regions.len().checked_mul(3)?)} else {4096};
    let mut count = 0usize;
    let mut wholly_left = 0usize;
    walk(&mut |edge| {
        count = count.checked_add(1)?;
        if right.is_some_and(|right|edge.a.x.max(edge.b.x)<right-1.) {
            wholly_left += 1;
        }
        if count > edge_limit {
            if std::env::var_os("RRRAH_TRACE_NATIVE_AREA_REFUSAL").is_some() {
                eprintln!("POLYGON_REFUSAL active_edges={count} region={} path_elements={} wholly_left={wholly_left}",edge.region,regions[edge.region].0.elements().len());
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
    if std::env::var_os("RRRAH_TRACE_POLYGON_DUPLICATES").is_some() {
        // Observation only: independent bounded pair scan, no retained storage
        // and no change to geometry, routing, or their work allowances.
        let mut diagnostic_work=8_000_000usize;let mut duplicates=0usize;let mut directed_duplicates=0usize;let mut complete=true;
        'diagnostic: for (index,edge) in edges.iter().enumerate() {
            if cancelled() {return None;}
            for other in &edges[..index] {
                let Some(left)=diagnostic_work.checked_sub(1) else {complete=false;break 'diagnostic;};
                diagnostic_work=left;
                if (edge.a==other.a && edge.b==other.b) || (edge.a==other.b && edge.b==other.a) {
                    duplicates+=1;
                    if edge.a==other.a && edge.b==other.b {directed_duplicates+=1;}
                    break;
                }
            }
        }
        eprintln!("POLYGON_EDGE_DUPLICATES edges={} duplicates={duplicates} first_match_directed={directed_duplicates} complete={complete} diagnostic_work={}",edges.len(),8_000_000-diagnostic_work);
    }
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
    crossings.sort_unstable_by(|a,b|a.0.total_cmp(&b.0).then_with(||a.1.total_cmp(&b.1)));
    // Identical intersections contribute the same cut; winding is carried by
    // edges, never by this list. Retain the original allocation admission.
    work.step(crossings.len())?;
    crossings.dedup_by(|a,b|a.0==b.0 && a.1==b.1);
    Some(PolygonRow {
        top,bottom,right,
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
        self.pixel_area_with_work::<false>(regions, x, y, &mut work, admit, None, None, None, None,0,None)
    }

    /// Visit disjoint convex fragments of the intersection. Uses the same
    /// admitted scratch and bounded edge/crossing work as `pixel_area`.
    /// Callback work is the caller's responsibility; callback refusal aborts
    /// the visit. Discard callback accumulation on None: earlier fragments may
    /// already have been visited. No fragment array is allocated or retained.
    pub(super) fn pixel_fragments(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,work_limit:usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>64 || x.abs_diff(0)>1_048_576 || y.abs_diff(0)>1_048_576 {return None;}
        let mut work=Work {left:work_limit,cancelled};
        work.step(1)?;
        self.pixel_area_with_work::<false>(regions,x,y,&mut work,admit,Some(visit),None,None,None,0,None)
    }

    /// Partition coverage by a caller-selected owner instead of intersecting
    /// every region. The selector sees original signed winding counts and must
    /// apply the declared fill rules and clip conditions. Some(None) excludes
    /// the strip; None refuses. A selected owner must be inside its own region.
    /// Source-order selection belongs to the caller. No owner array is retained.
    /// Discard callback accumulation if this method returns None.
    pub(super) fn pixel_selected_fragments(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,work_limit:usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        select:&mut dyn FnMut(&[i32])->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576 || y.abs_diff(0)>1_048_576 {return None;}
        let mut work=Work {left:work_limit,cancelled};
        work.step(1)?;
        self.pixel_area_with_work::<false>(regions,x,y,&mut work,admit,Some(visit),Some(select),None,None,0,None)
    }

    /// Incremental selector: None rebuilds at each slab; Some(region) updates
    /// exactly one winding after crossing an edge, including coincident edges.
    /// Caller declares bounded update cost; initialization charges all regions.
    /// Same refusal, owner-validation and discard-on-failure contract as above.
    pub(super) fn pixel_incremental_selected_fragments(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,work_limit:usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        self.pixel_incremental_selected_fragments_budget(regions,x,y,&mut {work_limit},cancelled,admit,update_cost,select,visit)
    }

    pub(super) fn pixel_incremental_selected_fragments_budget(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,remaining:&mut usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576
            || y.abs_diff(0)>1_048_576 || !(1..=4096).contains(&update_cost) {return None;}
        let mut work=Work {left:*remaining,cancelled};
        let result=(|| {
            work.step(1)?;
            self.pixel_area_with_work::<false>(regions,x,y,&mut work,admit,Some(visit),None,Some((select,update_cost)),None,0,None)
        })();
        *remaining=work.left;result
    }

    /// Integrate a subrectangle contained in this global pixel. Areas remain
    /// in global pixel units, so disjoint subrectangles can be added directly.
    pub(super) fn pixel_incremental_selected_rect_fragments(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,rect:kurbo::Rect,work_limit:usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576
            || y.abs_diff(0)>1_048_576 || !(1..=4096).contains(&update_cost) {return None;}
        if [rect.x0,rect.y0,rect.x1,rect.y1].iter().any(|v|!v.is_finite())
            || rect.x0<f64::from(x) || rect.x1>f64::from(x)+1.
            || rect.y0<f64::from(y) || rect.y1>f64::from(y)+1.
            || rect.width()<=0. || rect.height()<=0. {return None;}
        let mut work=Work {left:work_limit,cancelled};work.step(1)?;
        self.pixel_area_with_work::<false>(regions,x,y,&mut work,admit,Some(visit),None,
            Some((select,update_cost)),Some(rect),0,None)
    }

    pub(super) fn pixel_incremental_selected_rect_fragments_budget(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,rect:kurbo::Rect,remaining:&mut usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576
            || y.abs_diff(0)>1_048_576 || !(1..=4096).contains(&update_cost) {return None;}
        if [rect.x0,rect.y0,rect.x1,rect.y1].iter().any(|v|!v.is_finite())
            || rect.x0<f64::from(x) || rect.x1>f64::from(x)+1.
            || rect.y0<f64::from(y) || rect.y1>f64::from(y)+1.
            || rect.width()<=0. || rect.height()<=0. {return None;}
        let mut work=Work {left:*remaining,cancelled};
        let result=(|| {
            work.step(1)?;
            self.pixel_area_with_work::<false>(regions,x,y,&mut work,admit,Some(visit),None,
                Some((select,update_cost)),Some(rect),0,None)
        })();
        *remaining=work.left;result
    }

    /// Prepay every callback quota from the same finite geometry allowance.
    /// Unused local work is not refunded. Callback refusal discards the whole
    /// traversal result, and a callback may not enlarge its prepaid quota.
    pub(super) fn pixel_rect_fragments_with_callback_budget(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,rect:kurbo::Rect,remaining:&mut usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,visitor_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment,&mut usize)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576
            || y.abs_diff(0)>1_048_576 || !(1..=4096).contains(&update_cost)
            || !(1..=1_000_000).contains(&visitor_cost) {return None;}
        if [rect.x0,rect.y0,rect.x1,rect.y1].iter().any(|v|!v.is_finite())
            || rect.x0<f64::from(x) || rect.x1>f64::from(x)+1.
            || rect.y0<f64::from(y) || rect.y1>f64::from(y)+1.
            || rect.width()<=0. || rect.height()<=0. {return None;}
        let mut work=Work {left:*remaining,cancelled};
        let mut wrapped=|fragment| {
            let mut local=visitor_cost;
            let result=visit(fragment,&mut local);
            if local>visitor_cost {return None;}result
        };
        let result=(|| {
            work.step(1)?;
            self.pixel_area_with_work::<true>(regions,x,y,&mut work,admit,Some(&mut wrapped),None,
                Some((select,update_cost)),Some(rect),visitor_cost,None)
        })();
        *remaining=work.left;result
    }

    /// First slab initializes all regions; later slabs initialize only this tile's
    /// immutable edge-region subset. Other winding entries remain exactly zero.
    pub(super) fn pixel_rect_fragments_with_subset_callback_budget(
        &self,regions:&[(&BezPath,Fill)],x:i32,y:i32,rect:kurbo::Rect,remaining:&mut usize,
        cancelled:&dyn Fn()->bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
        update_cost:usize,visitor_cost:usize,
        select:&mut dyn FnMut(&[i32],Option<usize>,Option<&[usize]>)->Option<Option<usize>>,
        visit:&mut dyn FnMut(PixelFragment,&mut usize)->Option<()>,
    )->Option<f64> {
        if regions.is_empty() || regions.len()>4096 || x.abs_diff(0)>1_048_576
            || y.abs_diff(0)>1_048_576 || !(1..=4096).contains(&update_cost)
            || !(1..=1_000_000).contains(&visitor_cost) {return None;}
        if [rect.x0,rect.y0,rect.x1,rect.y1].iter().any(|v|!v.is_finite())
            || rect.x0<f64::from(x) || rect.x1>f64::from(x)+1.
            || rect.y0<f64::from(y) || rect.y1>f64::from(y)+1.
            || rect.width()<=0. || rect.height()<=0. {return None;}
        let mut work=Work {left:*remaining,cancelled};
        let mut wrapped=|fragment| {
            let mut local=visitor_cost;
            let result=visit(fragment,&mut local);
            if local>visitor_cost {return None;}result
        };
        let mut unused=|_:&[i32],_:Option<usize>|Some(None);
        let result=(|| {
            work.step(1)?;
            self.pixel_area_with_work::<true>(regions,x,y,&mut work,admit,Some(&mut wrapped),None,
                Some((&mut unused,update_cost)),Some(rect),visitor_cost,Some(select))
        })();
        *remaining=work.left;result
    }

    fn pixel_area_with_work<const PREFUND_VISITOR:bool>(
        &self,
        regions: &[(&BezPath, Fill)],
        x: i32,
        y: i32,
        work: &mut Work<'_>,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
        visit: Option<&mut dyn FnMut(PixelFragment)->Option<()>>,
        select: Option<&mut dyn FnMut(&[i32])->Option<Option<usize>>>,
        incremental: Option<(&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,usize)>,
        bounds: Option<kurbo::Rect>,
        visitor_cost:usize,
        subset_select:Option<&mut dyn FnMut(&[i32],Option<usize>,Option<&[usize]>)->Option<Option<usize>>>,
    ) -> Option<f64> {
        self.pixel_area_with_work_policy::<PREFUND_VISITOR>(regions,x,y,work,admit,visit,select,incremental,bounds,visitor_cost,subset_select,
            std::env::var_os("RRRAH_AI_LEFT_EDGE_EVENTS").is_some())
    }

    fn pixel_area_with_work_policy<const PREFUND_VISITOR:bool>(
        &self,
        regions: &[(&BezPath, Fill)],
        x: i32,
        y: i32,
        work: &mut Work<'_>,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
        mut visit: Option<&mut dyn FnMut(PixelFragment)->Option<()>>,
        mut select: Option<&mut dyn FnMut(&[i32])->Option<Option<usize>>>,
        mut incremental: Option<(&mut dyn FnMut(&[i32],Option<usize>)->Option<Option<usize>>,usize)>,
        bounds: Option<kurbo::Rect>,
        visitor_cost:usize,
        mut subset_select:Option<&mut dyn FnMut(&[i32],Option<usize>,Option<&[usize]>)->Option<Option<usize>>>,
        left_events:bool,
    ) -> Option<f64> {
        if y != self.y || regions.len() != self.regions {
            return None;
        }
        let cancelled = work.cancelled;
        let rect=bounds.unwrap_or(kurbo::Rect::new(f64::from(x),f64::from(y),f64::from(x)+1.,f64::from(y)+1.));
        let (left,right,top,bottom)=(rect.x0,rect.x1,rect.y0,rect.y1);
        if top<self.top || bottom>self.bottom || self.right.is_some_and(|limit|right>limit || left<limit-1.) {return None;}
        // Row edges are sorted by their minimum X. Edges wholly right of
        // this pixel affect neither its initial winding nor interior hits.
        // Retain left edges: they still determine winding inside the pixel.
        work.step(32)?;
        let end = self.edges.partition_point(|edge|edge.a.x.min(edge.b.x) <= right);
        let edges = &self.edges[..end];
        // Experimental tiled traversal: preserve original edge order and keep
        // boundary-touching edges. Crossings carry coordinates, not edge IDs.
        let filtered=if bounds.is_some() && std::env::var_os("RRRAH_AI_TILE_Y_EDGES").is_some() {
            Some(admitted_tile_y_edges(edges,top,bottom,work,admit)?)
        } else {None};
        let edges=filtered.as_ref().map_or(edges,|v|v.edges.as_slice());
        let count = edges.len();
        if count == 0 {return Some(0.);}
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
        let subset_bytes=if subset_select.is_some() {regions.len().checked_mul(size_of::<usize>())?} else {0};
        let bytes = cut_count
            .checked_mul(size_of::<f64>())?
            .checked_add(count.checked_mul(2*size_of::<(f64, usize)>()+5*size_of::<usize>())?)?
            .checked_add(count.div_ceil(64).checked_mul(size_of::<u64>())?)?
            .checked_add(regions.len().checked_mul(size_of::<i32>())?)?;
        let left_bytes=if left_events {regions.len().checked_mul(size_of::<i32>())?.checked_add(count.div_ceil(64).checked_mul(size_of::<u64>())?)?} else {0};
        let scratch_credit = admit(bytes.checked_add(subset_bytes)?.checked_add(left_bytes)?)?;
        if cancelled() {
            return None;
        }
        let mut subset=Vec::new();
        if subset_select.is_some() {
            subset.try_reserve_exact(regions.len()).ok()?;
            if subset.capacity()>regions.len() {return None;}
            let mut bits=[0u64;64];
            for edge in edges {work.step(1)?;bits[edge.region/64]|=1u64<<(edge.region%64);}
            work.step(bits.len())?;
            for index in active_edge_indices(&bits) {work.step(1)?;subset.push(index);}
        }
        let mut left_bits=Vec::new();let mut left_winding=Vec::new();
        if left_events {
            left_bits.try_reserve_exact(count.div_ceil(64)).ok()?;left_bits.resize(count.div_ceil(64),0u64);
            left_winding.try_reserve_exact(regions.len()).ok()?;left_winding.resize(regions.len(),0i32);
            if left_bits.capacity()>count.div_ceil(64) || left_winding.capacity()>regions.len() {return None;}
            for (index,&edge) in edges.iter().enumerate() {
                if edge_computed_x_stays_left(edge,top,bottom,left,work)? {left_bits[index/64]|=1u64<<(index%64);}
            }
        }
        let mut left_started=0usize;let mut left_ended=0usize;
        let mut first_slab=true;
        let mut cuts = Vec::new();
        cuts.try_reserve_exact(cut_count).ok()?;
        let mut hits = Vec::new();
        hits.try_reserve_exact(count).ok()?;
        let mut ordered_hits=Vec::new();ordered_hits.try_reserve_exact(count).ok()?;
        let mut hit_ranks=Vec::new();hit_ranks.try_reserve_exact(count).ok()?;hit_ranks.resize(count,usize::MAX);
        let mut previous_hit_len=usize::MAX;
        let mut edge_order=Vec::new();edge_order.try_reserve_exact(count).ok()?;
        edge_order.extend(0..count);
        let key=|index:usize| {
            let edge=edges[index];let (a,b)=if edge.a.y<edge.b.y || (edge.a.y==edge.b.y && edge.a.x<=edge.b.x) {(edge.a,edge.b)} else {(edge.b,edge.a)};
            [a.x,a.y,b.x,b.y]
        };
        work.step(count.checked_mul(16)?)?;
        edge_order.sort_unstable_by(|&a,&b| {
            let ka=key(a);let kb=key(b);
            ka.iter().zip(kb).map(|(a,b)|a.total_cmp(&b)).find(|v|!v.is_eq()).unwrap_or_else(||a.cmp(&b))
        });
        let mut edge_heads=Vec::new();edge_heads.try_reserve_exact(count).ok()?;edge_heads.resize(count,usize::MAX);
        let mut edge_next=Vec::new();edge_next.try_reserve_exact(count).ok()?;edge_next.resize(count,usize::MAX);
        let mut head=usize::MAX;let mut previous=usize::MAX;
        for &index in &edge_order {
            work.step(1)?;
            if previous==usize::MAX || key(index).map(f64::to_bits)!=key(previous).map(f64::to_bits) {head=index;}
            else {edge_next[previous]=index;}
            edge_heads[index]=head;previous=index;
        }
        // Reuse the completed canonical ordering as a Y-start index. Active
        // bits enumerate source edge indices in the same order as the full scan.
        work.step(count.checked_mul(32)?)?;
        edge_order.sort_unstable_by(|&a,&b|edges[a].low().total_cmp(&edges[b].low()).then(a.cmp(&b)));
        let mut end_order=Vec::new();end_order.try_reserve_exact(count).ok()?;
        end_order.extend(0..count);
        end_order.sort_unstable_by(|&a,&b|edges[a].high().total_cmp(&edges[b].high()).then(a.cmp(&b)));
        if cancelled() {return None;}
        let mut active=Vec::new();active.try_reserve_exact(count.div_ceil(64)).ok()?;
        active.resize(count.div_ceil(64),0u64);
        let mut started=0;let mut ended=0;
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
        if std::env::var_os("RRRAH_TRACE_POLYGON_DUPLICATES").is_some() && count>500 {
            eprintln!("POLYGON_CUT_DUPLICATES edges={count} emitted={cut_count} unique={}",cuts.len());
        }
        let certify_centroid=std::env::var_os("RRRAH_AI_TRACE_CENTROID_ENCLOSURE").is_some();
        let mut centroid_checks=0usize;let mut centroid_error=[0f64;2];let mut centroid_endpoint_repair=0f64;let mut edge_rounding=0f64;
        let trace_sweep=std::env::var_os("RRRAH_AI_TRACE_SWEEP_WORK").is_some();
        let mut sweep_counts=[0usize;5];
        let mut area = 0.;
        let mut compensation = 0.;
        for slab in cuts.windows(2) {
            if trace_sweep {sweep_counts[0]+=1;sweep_counts[1]+=if subset_select.is_some() && !first_slab {subset.len()} else {regions.len()};}
            work.step(1)?;
            let (a, b) = (slab[0], slab[1]);
            if b <= a {
                return None;
            }
            // Consecutive representable Y values can have no representable midpoint.
            // Evaluate linear X at both endpoints instead of inventing/dropping a slab.

            while started<count && edges[edge_order[started]].low()<b {
                work.step(1)?;let index=edge_order[started];
                if !left_events || left_bits[index/64]&(1u64<<(index%64))==0 {active[index/64]|=1u64<<(index%64);}
                started+=1;
            }
            while ended<count && edges[end_order[ended]].high()<=a {
                work.step(1)?;let index=end_order[ended];active[index/64]&=!(1u64<<(index%64));ended+=1;
            }
            hits.clear();
            if left_events {
                let midpoint=a*0.5+b*0.5;let sample=if midpoint<b {midpoint} else {a};
                while left_started<count && edges[edge_order[left_started]].low()<=sample {
                    work.step(1)?;let index=edge_order[left_started];let edge=edges[index];
                    if left_bits[index/64]&(1u64<<(index%64))!=0 {left_winding[edge.region]=left_winding[edge.region].checked_add(edge.winding())?;}
                    left_started+=1;
                }
                while left_ended<count && edges[end_order[left_ended]].high()<=sample {
                    work.step(1)?;let index=end_order[left_ended];let edge=edges[index];
                    if left_bits[index/64]&(1u64<<(index%64))!=0 {left_winding[edge.region]=left_winding[edge.region].checked_sub(edge.winding())?;}
                    left_ended+=1;
                }
                winding.copy_from_slice(&left_winding);
            } else {winding.fill(0);}
            work.step(active.len())?;
            for index in active_edge_indices(&active) {
                if trace_sweep {sweep_counts[2]+=1;}
                let edge=edges[index];
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
                    } else if x < right && edge_heads[index]==index {
                        hits.push((x, index));
                    }
                }
            }
            order_slab_hits(&mut hits,&mut ordered_hits,&mut hit_ranks,&mut previous_hit_len,work)?;
            if trace_sweep {sweep_counts[3]+=hits.len();}
            // Distinct lines tied at the sampled X retain legacy ordering.
            // Identical canonical geometry can otherwise share one hit while
            // each member still updates its own region winding below.
            let grouped=!hits.windows(2).any(|pair|pair[0].0==pair[1].0);
            if !grouped {
                hits.clear();
                work.step(active.len())?;
                for index in active_edge_indices(&active) {
                    let edge=edges[index];
                    work.step(1)?;
                    if edge.low()<=a && edge.high()>=b {
                        let x=edge.x(a)*0.5+edge.x(b)*0.5;
                        if x>left && x<right {hits.push((x,index));}
                    }
                }
                work.step(hits.len().checked_mul(16)?)?;
                hits.sort_unstable_by(|a,b|a.0.total_cmp(&b.0));
            }
            // Edges wholly left of this slab contribute only the initial winding;
            // edges wholly right cannot affect the pixel. Sort only interior hits.
            let mut incremental_owner = if let Some((selector,cost))=incremental.as_mut() {
                let initial=if first_slab {None} else {Some(subset.as_slice())};
                work.step(if subset_select.is_some() && !first_slab {subset.len()} else {regions.len()}.checked_add(*cost)?)?;
                if let Some(sparse)=subset_select.as_mut() {sparse(&winding,None,initial)?}
                else {selector(&winding,None)?}
            } else {None};
            first_slab=false;
            let mut previous = None;
            for next in hits.iter().map(|hit| Some(hit.1)).chain(std::iter::once(None)) {
                work.step(1)?;
                let xa = previous.map_or(left, |index: usize| edges[index].x(a).max(left));
                let xb = previous.map_or(left, |index: usize| edges[index].x(b).max(left));
                let ya = next.map_or(right, |index| edges[index].x(a).min(right));
                let yb = next.map_or(right, |index| edges[index].x(b).min(right));
                let contribution = ((ya-xa).max(0.)+(yb-xb).max(0.))*0.5*(b-a);
                if !contribution.is_finite() {return None;}
                let inside=|r:usize|match regions[r].1 {
                    Fill::NonZero=>winding[r]!=0,Fill::EvenOdd=>winding[r]%2!=0,
                };
                // Coincident edges define no covered strip. Preserve winding
                // updates below, but avoid scanning every region for zero area.
                let owner=if contribution == 0. {None} else if incremental.is_some() {
                    incremental_owner
                } else if let Some(selector)=select.as_mut() {
                    work.step(regions.len())?;
                    let owner=selector(&winding)?;
                    if owner.is_some_and(|r|r>=regions.len() || !inside(r)) {return None;}
                    owner
                } else {
                    work.step(regions.len())?;
                    (0..regions.len()).all(inside).then_some(0)
                };
                if let Some(owner)=owner {
                    if owner>=regions.len() || !inside(owner) {return None;}
                    if contribution>0. {
                        if let Some(visitor)=visit.as_mut() {
                            if PREFUND_VISITOR {work.step(visitor_cost)?;}
                            // Local X and normalized widths avoid subtracting
                            // large squared coordinates or dividing by a tiny slab height.
                            let wa=(ya-xa).max(0.);let wb=(yb-xb).max(0.);
                            let u=wa/(wa+wb);let v=1.-u;
                            let ma=xa-left+wa*0.5;let mb=xb-left+wb*0.5;
                            let centroid=Point::new(left+((2.*u+v)*ma+(u+2.*v)*mb)/3.,a+(b-a)*(u+2.*v)/3.);
                            if cancelled() || !centroid.x.is_finite() || !centroid.y.is_finite() {return None;}
                            let vertices=[Point::new(xa,a),Point::new(ya,a),Point::new(yb,b),Point::new(xb,b)];
                            if certify_centroid {
                                for (boundary,row,value,is_left) in [(previous,a,xa,true),(next,a,ya,false),(next,b,yb,false),(previous,b,xb,true)] {
                                    let interval=if let Some(index)=boundary {edge_x_enclosure(edges[index],row,work)?}
                                        else if is_left {[left,left]} else {[right,right]};
                                    let interval=if is_left {interval.map(|x|x.max(left))} else {interval.map(|x|x.min(right))};
                                    work.step(1)?;
                                    if value<interval[0] || value>interval[1] {return None;}
                                    let error=(value-interval[0]).max(interval[1]-value);
                                    edge_rounding=edge_rounding.max(if error==0. {0.} else {error.next_up()});
                                }
                                let bounds=trapezoid_centroid_enclosure(vertices,left,work).or_else(|| {eprintln!("POLYGON_CENTROID_ENCLOSURE_REFUSAL vertices={vertices:?} left={left:?} remaining={}",work.left);None})?;
                                work.step(2)?;
                                for (axis,value) in [centroid.x,centroid.y].into_iter().enumerate() {
                                    if value<bounds[axis][0] || value>bounds[axis][1] {eprintln!("POLYGON_CENTROID_ENCLOSURE_MISS axis={axis} value={value:?} bounds={:?}",bounds[axis]);return None;}
                                    centroid_error[axis]=centroid_error[axis].max((value-bounds[axis][0]).max(bounds[axis][1]-value).next_up());
                                }
                                let repair=(xa-ya).max(0.).max((xb-yb).max(0.));
                                centroid_endpoint_repair=centroid_endpoint_repair.max(if repair==0. {0.} else {repair.next_up()});
                                centroid_checks=centroid_checks.checked_add(1)?;
                            }
                            visitor(PixelFragment {
                                vertices,
                                area:contribution,centroid,owner,
                            })?;
                            if cancelled() {return None;}
                        }
                    }
                    let corrected = contribution - compensation;
                    let sum = area + corrected;
                    compensation = (sum - area) - corrected;
                    area = sum;
                }
                if let Some(index) = next {
                    let mut member=index;
                    loop {
                        if member!=index {work.step(1)?;}
                        let edge=edges[member];
                        if trace_sweep {sweep_counts[4]+=1;}
                        winding[edge.region]+=edge.winding();
                        if let Some((selector,cost))=incremental.as_mut() {
                            work.step(*cost)?;
                            incremental_owner=if let Some(sparse)=subset_select.as_mut() {sparse(&winding,Some(edge.region),None)?}
                                else {selector(&winding,Some(edge.region))?};
                        }
                        if !grouped || edge_next[member]==usize::MAX {break;}
                        member=edge_next[member];
                    }
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
        if trace_sweep {eprintln!("POLYGON_SWEEP_WORK slabs={} initial_region_slots={} active_edge_visits={} grouped_hits={} winding_updates={} geometry_remaining={}",sweep_counts[0],sweep_counts[1],sweep_counts[2],sweep_counts[3],sweep_counts[4],work.left);}
        drop(left_bits);drop(left_winding);
        drop(subset);
        drop(winding);
        drop(hits);
        drop(edge_order);drop(edge_heads);drop(edge_next);
        drop(ordered_hits);drop(hit_ranks);
        drop(cuts);
        drop(scratch_credit);
        if certify_centroid {eprintln!("POLYGON_CENTROID_ENCLOSURE checks={centroid_checks} max_rounding={centroid_error:?} max_endpoint_repair={centroid_endpoint_repair:?} max_edge_rounding={edge_rounding:?}");}
        Some(area.clamp(0., 1.))
    }
}

// With a fixed finite slope, IEEE subtract/multiply/add are monotone in Y
// (reversed for a negative slope). Endpoint evaluations therefore enclose
// every computed edge X in this clipped lifetime. The averaging bound also
// requires left/2 + left/2 == left. This proves equivalence to this floating
// kernel, not enclosure of the ideal source geometry.
fn edge_computed_x_stays_left(edge:Edge,top:f64,bottom:f64,left:f64,work:&mut Work<'_>)->Option<bool> {
    work.step(24)?;
    let low=top.max(edge.low());let high=bottom.min(edge.high());
    if low>high || !left.is_finite() || left*0.5+left*0.5!=left {return Some(false);}
    let a=edge.x(low);let b=edge.x(high);
    Some(a.is_finite() && b.is_finite() && a<=left && b<=left)
}

// Reuse a previous strict ordering only after proving identical membership
// and strict current X order after bounded adjacent-crossing repair. Ties or
// excessive disorder use the original unstable sort and its exact tie behavior.
fn order_slab_hits(hits:&mut Vec<(f64,usize)>,ordered:&mut Vec<(f64,usize)>,ranks:&mut [usize],
    previous_len:&mut usize,work:&mut Work<'_>)->Option<()> {
    let mut reusable=hits.len()==*previous_len;
    if reusable {
        work.step(hits.len().checked_mul(2)?)?;
        ordered.clear();ordered.resize(hits.len(),(0.,usize::MAX));
        for &hit in hits.iter() {
            let rank=*ranks.get(hit.1)?;
            if rank>=hits.len() {reusable=false;break;}
            ordered[rank]=hit;
        }
        if reusable {
            // Adjacent crossing cuts usually exchange only a few neighbors.
            // Repair that permutation with a bounded insertion pass. Preserve
            // the original input for exact fallback on ties or excessive swaps.
            let mut swaps=0usize;let maximum=hits.len().checked_mul(2)?;
            'repair: for index in 1..ordered.len() {
                let mut at=index;
                while at>0 {
                    work.step(1)?;
                    if ordered[at-1].0<ordered[at].0 {break;}
                    if ordered[at-1].0==ordered[at].0 || swaps==maximum {reusable=false;break 'repair;}
                    work.step(1)?;ordered.swap(at-1,at);swaps+=1;at-=1;
                }
            }
            if reusable && swaps>0 {
                work.step(ordered.len())?;
                for (rank,&(_,index)) in ordered.iter().enumerate() {*ranks.get_mut(index)?=rank;}
            }
        }
    }
    if reusable {
        std::mem::swap(hits,ordered);
    } else {
        work.step(hits.len().checked_mul(16)?)?;
        hits.sort_unstable_by(|a,b|a.0.total_cmp(&b.0));
        work.step(ranks.len().checked_add(hits.len())?)?;
        ranks.fill(usize::MAX);
        for (rank,&(_,index)) in hits.iter().enumerate() {*ranks.get_mut(index)?=rank;}
        *previous_len=hits.len();
    }
    if (work.cancelled)() {return None;}
    Some(())
}

#[test]
fn slab_order_reuse_matches_original_sort_across_membership_crossings_and_ties() {
    let mut ordered=Vec::with_capacity(8);let mut ranks=[usize::MAX;8];let mut len=usize::MAX;
    let mut work=Work {left:100_000,cancelled:&||false};
    for input in [vec![(0.3f64,2usize),(0.1,0),(0.2,1)],vec![(0.32,2),(0.12,0),(0.22,1)],
        vec![(0.1,2),(0.3,0),(0.2,1)],vec![(0.1,2),(0.1,0),(0.2,1)],
        vec![(0.1,3),(0.3,0),(0.2,1)],vec![(0.1,3)],vec![],vec![],vec![(0.4,4),(0.2,2)]] {
        let mut expected=input.clone();expected.sort_unstable_by(|a,b|a.0.total_cmp(&b.0));
        let mut hits=input;
        order_slab_hits(&mut hits,&mut ordered,&mut ranks,&mut len,&mut work).unwrap();
        assert_eq!(hits,expected);
    }
    // Deterministic varying membership, input permutation, near crossings,
    // large disorder and ties; compare the complete (X,index) sequence.
    let mut ranks=[usize::MAX;64];let mut len=usize::MAX;
    let mut ordered=Vec::with_capacity(64);
    let mut work=Work {left:usize::MAX,cancelled:&||false};
    let mut state=17u64;
    for step in 0..1024 {
        let mut hits=Vec::new();
        for index in 0..64 {
            state=state.wrapping_mul(6364136223846793005).wrapping_add(1);
            if step%7==0 && state%5==0 {continue;}
            let x=match step%4 {
                0=>index as f64+(state%3) as f64*0.75,
                1=>(state%8) as f64,
                2=>(state%128) as f64*0.5,
                _=>index as f64,
            };
            hits.push((x,index));
        }
        if !hits.is_empty() {let rotate=step%hits.len();hits.rotate_left(rotate);}
        let mut expected=hits.clone();expected.sort_unstable_by(|a,b|a.0.total_cmp(&b.0));
        order_slab_hits(&mut hits,&mut ordered,&mut ranks,&mut len,&mut work).unwrap();
        assert_eq!(hits,expected,"step {step}");
    }
    let mut hits=vec![(0.1,0),(0.2,1)];len=usize::MAX;
    assert!(order_slab_hits(&mut hits,&mut ordered,&mut ranks,&mut len,&mut Work {left:0,cancelled:&||false}).is_none());
    assert!(order_slab_hits(&mut hits,&mut ordered,&mut ranks,&mut len,&mut Work {left:1000,cancelled:&||true}).is_none());
}

/// Count and then emit the same subdivision cuts. Disjoint segment X ranges
/// cannot cross, so they need no line-intersection calculation.
// A set bit denotes an edge whose open Y interval intersects the slab.
// Inactive edges cannot affect either the old full-span or midpoint winding rule.
fn active_edge_indices(bits:&[u64])->impl Iterator<Item=usize> + '_ {
    bits.iter().copied().enumerate().flat_map(|(word,mut value)|std::iter::from_fn(move || {
        if value==0 {return None;}
        let index=value.trailing_zeros() as usize;value&=value-1;
        Some(word*64+index)
    }))
}
struct AdmittedTileEdges {
    edges:Vec<Edge>,
    _credit:Box<dyn std::any::Any+Send+Sync>,
}
fn admitted_tile_y_edges(edges:&[Edge],top:f64,bottom:f64,work:&mut Work<'_>,
    admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>)->Option<AdmittedTileEdges> {
    work.step(1)?;
    let overlaps=|edge:Edge|edge.low()<=bottom && edge.high()>=top;
    let mut count=0usize;
    for &edge in edges {work.step(1)?;if overlaps(edge) {count=count.checked_add(1)?;}}
    let credit=admit(count.checked_mul(size_of::<Edge>())?)?;
    let mut selected=Vec::new();selected.try_reserve_exact(count).ok()?;
    if selected.capacity()>count {return None;}
    for &edge in edges {work.step(1)?;if overlaps(edge) {selected.push(edge);}}
    work.step(1)?;Some(AdmittedTileEdges {edges:selected,_credit:credit})
}

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
    visit_crossings_with_shared_vertex_policy(edges,top,bottom,work,out,
        std::env::var_os("RRRAH_AI_SKIP_SHARED_VERTEX_CROSSINGS").is_some(),
        std::env::var_os("RRRAH_AI_AUDIT_PROPER_CROSSINGS").is_some())
}
fn edges_share_vertex(a:Edge,b:Edge)->bool {
    a.a==b.a || a.a==b.b || a.b==b.a || a.b==b.b
}
fn visit_crossings_with_shared_vertex_policy(
    edges: &[Edge],
    top: f64,
    bottom: f64,
    work: &mut Work<'_>,
    out: &mut dyn FnMut((f64, f64)) -> Option<()>,
    skip_shared_vertices:bool,
    audit_proper_crossings:bool,
) -> Option<()> {
    let audit=std::env::var_os("RRRAH_AI_TRACE_CROSSING_ENCLOSURE").is_some();
    let mut predicate_checks=0usize;let mut certified_nonproper=0usize;let mut predicate_ambiguous=0usize;
    let mut bounded=0usize;let mut boundary_ambiguous=0usize;let mut arithmetic_ambiguous=0usize;let mut max_rounding=0f64;
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
            // Straight segments sharing an exact endpoint have no distinct
            // interior intersection (collinear overlap has no unique crossing).
            // Keep every source edge/winding update; skip only the false cut.
            if skip_shared_vertices && edges_share_vertex(edge,other) {continue;}
            let denominator = slope - other.slope();
            if lo < hi && denominator != 0. {
                let y = lo + (other.x(lo) - edge.x(lo)) / denominator;
                if y > lo && y < hi {
                    if audit_proper_crossings {
                        predicate_checks=predicate_checks.checked_add(1)?;
                        match proper_segment_crossing(edge,other,work) {
                            Some(false)=>{certified_nonproper=certified_nonproper.checked_add(1)?;}
                            Some(true)=>{},
                            None=>{
                                if work.left==0 || (work.cancelled)() {return None;}
                                predicate_ambiguous=predicate_ambiguous.checked_add(1)?;
                            },
                        }
                    }
                    let x = edge.x(y);
                    if !x.is_finite() {
                        return None;
                    }
                    if audit {
                        if let Some(interval)=crossing_y_enclosure(edge,other,lo,work) {
                            if y<interval[0] || y>interval[1] {return None;}
                            bounded=bounded.checked_add(1)?;
                            if interval[0]<=lo || interval[1]>=hi {boundary_ambiguous=boundary_ambiguous.checked_add(1)?;}
                            max_rounding=max_rounding.max((y-interval[0]).max(interval[1]-y).next_up());
                        } else {
                            if work.left==0 || (work.cancelled)() {return None;}
                            arithmetic_ambiguous=arithmetic_ambiguous.checked_add(1)?;
                        }
                    }
                    out((x, y))?;
                }
            }
        }
    }
    if audit_proper_crossings && predicate_checks>0 {eprintln!("POLYGON_PROPER_CROSSING_AUDIT checked={predicate_checks} nonproper={certified_nonproper} undecided={predicate_ambiguous}");}
    if audit && bounded+arithmetic_ambiguous>0 {eprintln!("POLYGON_CROSSING_ENCLOSURE bounded={bounded} boundary_ambiguous={boundary_ambiguous} arithmetic_ambiguous={arithmetic_ambiguous} max_rounding={max_rounding:?}");}
    Some(())
}

// A continuous chain strictly left of a pixel contributes only winding.
// Oriented crossings telescope to its endpoints even if Y is nonmonotone.
fn flush_left_chain(
    chain:&mut Option<(Point,Point)>,region:usize,top:f64,bottom:f64,right:Option<f64>,
    out:&mut dyn FnMut(Edge)->Option<()>,
)->Option<()> {
    if let Some((a,b))=chain.take() {
        let left=right?-1.;
        out_if_active(Point::new(left,a.y),Point::new(left,b.y),region,top,bottom,out)?;
    }
    Some(())
}
fn emit_row_edge(
    a:Point,b:Point,region:usize,top:f64,bottom:f64,right:Option<f64>,
    chain:&mut Option<(Point,Point)>,out:&mut dyn FnMut(Edge)->Option<()>,
)->Option<()> {
    if right.is_some_and(|r|a.x.max(b.x)<r-1.) {
        match chain {
            Some((_,last)) if *last==a=>*last=b,
            _=>{
                flush_left_chain(chain,region,top,bottom,right,out)?;
                *chain=Some((a,b));
            }
        }
        return Some(());
    }
    flush_left_chain(chain,region,top,bottom,right,out)?;
    if right.is_some_and(|r|a.x.min(b.x)>r) {return Some(());}
    out_if_active(a,b,region,top,bottom,out)
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
fn grouped_shared_edges_preserve_region_winding_and_even_odd_multiplicity() {
    let make=|reverse:bool| {
        let mut path=BezPath::new();path.move_to((0.,0.));
        for p in if reverse {[(0.,1.),(1.,0.)]} else {[(1.,0.),(0.,1.)]} {path.line_to(p);}
        path.close_path();path
    };
    let forward=make(false);let reverse=make(true);
    for first in [Fill::NonZero,Fill::EvenOdd] {for second in [Fill::NonZero,Fill::EvenOdd] {
        let regions=[(&forward,first),(&reverse,second)];
        assert_eq!(polygon_pixel_area(&regions,0,0,1_000_000,&||false,&|_|Some(Box::new(()))),Some(0.5));
    }}
    for (paths,nonzero,evenodd) in [(vec![&forward,&forward],0.5,0.),
        (vec![&forward,&reverse],0.,0.),(vec![&forward,&forward,&reverse],0.5,0.5)] {
        let mut repeated=BezPath::new();
        for path in paths {repeated.extend(path.elements().iter().copied());}
        for (fill,expected) in [(Fill::NonZero,nonzero),(Fill::EvenOdd,evenodd)] {
            assert_eq!(polygon_pixel_area(&[(&repeated,fill)],0,0,1_000_000,&||false,&|_|Some(Box::new(()))),Some(expected));
        }
    }
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

#[test]
fn pixel_fragments_integrate_affine_color_when_pixel_center_is_outside() {
    for (ox,oy) in [(0,0),(1_000_000,-1_000_000),(-1_000_000,1_000_000)] {
        let (x,y)=(f64::from(ox),f64::from(oy));
        let mut triangle=BezPath::new();
        triangle.move_to((x,y));triangle.line_to((x+0.375,y));triangle.line_to((x,y+1.));triangle.close_path();
        let regions=[(&triangle,Fill::NonZero)];
        let row=admitted_polygon_row(&regions,oy,1_000_000,&||false,&|_|Some(Box::new(()))).unwrap();
        let mut integral=[0.;4];let mut fragments=0;
        let area=row.pixel_fragments(&regions,ox,oy,1_000_000,&||false,&|_|Some(Box::new(())),&mut |fragment| {
            fragments+=1;
            assert!(fragment.area>0.);
            let local=fragment.centroid-Point::new(x,y);
            assert!(local.x/0.375+local.y<1.);
            assert!(fragment.vertices.iter().all(|p|p.x>=x && p.x<=x+1. && p.y>=y && p.y<=y+1.));
            let components=[0.2+0.2*local.x+0.3*local.y,0.1+0.4*local.x,0.6-0.3*local.y,0.5];
            for (sum,value) in integral.iter_mut().zip(components) {*sum+=fragment.area*value;}
            Some(())
        }).unwrap();
        assert_eq!(area,3./16.);
        assert_eq!(area,row.pixel_area(&regions,ox,oy,1_000_000,&||false,&|_|Some(Box::new(()))).unwrap());
        assert_eq!(fragments,1);
        for (actual,mean) in integral.into_iter().zip([0.325,0.15,0.5,0.5]) {
            assert!((actual-(3./16.)*mean).abs()<2e-11,"origin={ox},{oy} integral={actual}");
        }
        // Center is not in the triangle: the old color-at-center route loses all area.
        assert!(0.5/0.375+0.5>1.);
    }
}

#[test]
fn pixel_fragments_keep_disconnected_coverage_outside_an_evenodd_hole() {
    let mut ring=BezPath::new();
    for (a,b) in [(0.,1.),(0.25,0.75)] {
        ring.move_to((a,a));ring.line_to((b,a));ring.line_to((b,b));ring.line_to((a,b));ring.close_path();
    }
    let regions=[(&ring,Fill::EvenOdd)];
    let row=admitted_polygon_row(&regions,0,1_000_000,&||false,&|_|Some(Box::new(()))).unwrap();
    let mut integral=0.;let mut fragments=0;
    let area=row.pixel_fragments(&regions,0,0,1_000_000,&||false,&|_|Some(Box::new(())),&mut |fragment| {
        let p=fragment.centroid;
        assert!(p.x<=0.25 || p.x>=0.75 || p.y<=0.25 || p.y>=0.75);
        fragments+=1;integral+=fragment.area*(p.x+2.*p.y);Some(())
    }).unwrap();
    assert_eq!(area,0.75);assert_eq!(integral,1.125);assert_eq!(fragments,4);
}

#[test]
fn pixel_fragment_refusals_and_every_cancellation_release_scratch() {
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));let requests=Cell::new(0);let deny=Cell::new(0);
    let admit=|bytes| {
        requests.set(requests.get()+1);
        if requests.get()==deny.get() {return None;}
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
    };
    let mut triangle=BezPath::new();triangle.move_to((0.,0.));triangle.line_to((0.375,0.));triangle.line_to((0.,1.));triangle.close_path();
    let regions=[(&triangle,Fill::NonZero)];
    let row=admitted_polygon_row(&regions,0,1_000_000,&||false,&admit).unwrap();
    let retained=used.load(Ordering::Relaxed);assert!(retained>0);
    requests.set(0);let polls=Cell::new(0);
    assert_eq!(row.pixel_fragments(&regions,0,0,1_000_000,&||{polls.set(polls.get()+1);false},&admit,&mut |_|Some(())),Some(3./16.));
    assert_eq!(used.load(Ordering::Relaxed),retained);
    let total=requests.get();assert!(total>0);
    for stop in 1..=total {
        requests.set(0);deny.set(stop);
        assert!(row.pixel_fragments(&regions,0,0,1_000_000,&||false,&admit,&mut |_|Some(())).is_none());
        assert_eq!(used.load(Ordering::Relaxed),retained);
    }
    deny.set(0);
    for stop in 1..=polls.get() {
        let current=Cell::new(0);
        assert!(row.pixel_fragments(&regions,0,0,1_000_000,&||{current.set(current.get()+1);current.get()==stop},&admit,&mut |_|Some(())).is_none());
        assert_eq!(used.load(Ordering::Relaxed),retained);
    }
    assert!(row.pixel_fragments(&regions,0,0,1_000_000,&||false,&admit,&mut |_|None).is_none());
    assert!(row.pixel_fragments(&regions,0,0,2,&||false,&admit,&mut |_|Some(())).is_none());
    assert_eq!(used.load(Ordering::Relaxed),retained);
    println!("PIXEL_FRAGMENT_BUDGET reservations={total} cancellation_checkpoints={}",polls.get());
    drop(row);assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn selected_pixel_fragments_integrate_visible_triangles_in_source_order() {
    use kurbo::Shape;
    let whole=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let mut later=BezPath::new();later.move_to((0.,0.));later.line_to((1.,0.));later.line_to((0.,1.));later.close_path();
    for clip_bounds in [kurbo::Rect::new(0.,0.,1.,1.),kurbo::Rect::new(0.25,0.25,0.75,0.75)] {
        let clip=clip_bounds.to_path(0.1);
        let regions=[(&clip,Fill::NonZero),(&whole,Fill::NonZero),(&later,Fill::NonZero)];
        let row=admitted_polygon_row(&regions,0,1_000_000,&||false,&|_|Some(Box::new(()))).unwrap();
        for reverse in [false,true] {
            let mut owned=[0.;3];let mut cmyk=[0.;4];
            let area=row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&|_|Some(Box::new(())),&mut |w| {
                if w[0]==0 {return Some(None);}
                Some(if reverse {(1..3).find(|&r|w[r]!=0)}else{(1..3).rev().find(|&r|w[r]!=0)})
            },&mut |fragment| {
                owned[fragment.owner]+=fragment.area;
                let color=if fragment.owner==1 {[1.,0.,0.,0.]}else{[0.,1.,0.,0.]};
                for (component,value) in cmyk.iter_mut().zip(color) {*component+=fragment.area*value;}
                Some(())
            }).unwrap();
            let expected_area=clip_bounds.width()*clip_bounds.height();
            assert_eq!(area,expected_area);
            if reverse {assert_eq!(owned,[0.,expected_area,0.]);assert_eq!(cmyk,[expected_area,0.,0.,0.]);}
            else {assert_eq!(owned,[0.,expected_area/2.,expected_area/2.]);assert_eq!(cmyk,[expected_area/2.,expected_area/2.,0.,0.]);}
            assert_eq!(owned.iter().sum::<f64>(),area);
        }
    }
}

#[test]
fn selected_pixel_fragments_apply_evenodd_clip_before_source_union() {
    use kurbo::Shape;
    let whole=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let mut clip=whole.clone();
    clip.extend(kurbo::Rect::new(0.25,0.25,0.75,0.75).to_path(0.1).elements().iter().copied());
    let regions=[(&clip,Fill::EvenOdd),(&whole,Fill::NonZero)];
    let row=admitted_polygon_row(&regions,0,1_000_000,&||false,&|_|Some(Box::new(()))).unwrap();
    let mut integral=0.;
    let area=row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&|_|Some(Box::new(())),&mut |w| {
        Some((w[0]%2!=0 && w[1]!=0).then_some(1))
    },&mut |fragment| {
        assert_eq!(fragment.owner,1);
        integral+=fragment.area*(fragment.centroid.x+2.*fragment.centroid.y);Some(())
    }).unwrap();
    assert_eq!(area,0.75);assert_eq!(integral,1.125);
}

#[test]
fn selected_pixel_fragment_refusal_and_cancel_release_scratch() {
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    use kurbo::Shape;
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));let requests=Cell::new(0);let deny=Cell::new(0);
    let admit=|bytes| {
        requests.set(requests.get()+1);if requests.get()==deny.get() {return None;}
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
    };
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let regions=[(&path,Fill::NonZero)];
    let row=admitted_polygon_row(&regions,0,1_000_000,&||false,&admit).unwrap();
    let baseline=used.load(Ordering::Relaxed);requests.set(0);let polls=Cell::new(0);
    let mut select=|w:&[i32]|Some((w[0]!=0).then_some(0));
    assert_eq!(row.pixel_selected_fragments(&regions,0,0,1_000_000,&||{polls.set(polls.get()+1);false},&admit,&mut select,&mut |_|Some(())),Some(1.));
    let total=requests.get();
    for stop in 1..=total {
        requests.set(0);deny.set(stop);
        assert!(row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,&mut select,&mut |_|Some(())).is_none());
        assert_eq!(used.load(Ordering::Relaxed),baseline);
    }
    deny.set(0);
    for stop in 1..=polls.get() {
        let current=Cell::new(0);
        assert!(row.pixel_selected_fragments(&regions,0,0,1_000_000,&||{current.set(current.get()+1);current.get()==stop},&admit,&mut select,&mut |_|Some(())).is_none());
        assert_eq!(used.load(Ordering::Relaxed),baseline);
    }
    for mut refused in [Box::new(|_:&[i32]|None) as Box<dyn FnMut(&[i32])->Option<Option<usize>>>,Box::new(|_:&[i32]|Some(Some(1)))] {
        assert!(row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,&mut refused,&mut |_|Some(())).is_none());
        assert_eq!(used.load(Ordering::Relaxed),baseline);
    }
    assert!(row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,&mut select,&mut |_|None).is_none());
    assert!(row.pixel_selected_fragments(&regions,0,0,2,&||false,&admit,&mut select,&mut |_|Some(())).is_none());
    println!("SELECTED_FRAGMENT_BUDGET reservations={total} cancellation_checkpoints={}",polls.get());
    drop(row);assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn selected_pixel_regions_cover_real_candidate_count_without_lifting_ordinary_cap() {
    use kurbo::Shape;
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let regions=vec![(&path,Fill::NonZero);74];
    assert!(admitted_polygon_row(&regions,0,20_000_000,&||false,&|_|Some(Box::new(()))).is_none());
    let row=admitted_selected_polygon_row(&regions,0,20_000_000,&||false,&|_|Some(Box::new(()))).unwrap();
    let mut owned_area=0.;
    assert_eq!(row.pixel_selected_fragments(&regions,0,0,20_000_000,&||false,&|_|Some(Box::new(())),&mut |w| {
        Some(if w[0]!=0 {(1..w.len()).rev().find(|&r|w[r]!=0)}else{None})
    },&mut |fragment| {
        assert_eq!(fragment.owner,73);owned_area+=fragment.area;Some(())
    }),Some(1.));
    assert_eq!(owned_area,1.);
    let excessive=vec![(&path,Fill::NonZero);4097];
    let admissions=std::cell::Cell::new(0);
    assert!(admitted_selected_polygon_row(&excessive,0,20_000_000,&||false,&|_| {admissions.set(admissions.get()+1);Some(Box::new(()))}).is_none());
    assert_eq!(admissions.get(),0);
}

#[test]
fn coincident_edges_skip_zero_area_owner_scans() {
    use kurbo::Shape;
    let path=kurbo::Rect::new(0.25,0.,0.75,1.).to_path(0.1);
    let regions=vec![(&path,Fill::NonZero);300];
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_row(&regions,0,20_000_000,&||false,&admit).unwrap();
    let mut selections=0;let mut integrated=0.;
    let area=row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,
        &mut |w| {selections+=1;Some((0..w.len()).rev().find(|&i|w[i]!=0))},
        &mut |fragment| {assert_eq!(fragment.owner,299);integrated+=fragment.area;Some(())});
    assert_eq!(area,Some(0.5));assert_eq!(integrated,0.5);
    assert_eq!(selections,3,"only three positive-width strips require ownership");
}

#[test]
fn right_disjoint_edges_do_not_consume_pixel_subdivision_work() {
    use kurbo::Shape;
    let paint=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let outside=kurbo::Rect::new(10.,0.,11.,1.).to_path(0.1);
    let mut regions=vec![(&outside,Fill::NonZero);300];
    regions.push((&paint,Fill::NonZero));
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_row(&regions,0,20_000_000,&||false,&admit).unwrap();
    let mut area=0.;
    assert_eq!(row.pixel_selected_fragments(&regions,0,0,5000,&||false,&admit,
        &mut |w|Some((0..w.len()).rev().find(|&i|w[i]!=0)),
        &mut |fragment| {assert_eq!(fragment.owner,300);area+=fragment.area;Some(())}),Some(1.));
    assert_eq!(area,1.);
    assert_eq!(row.pixel_selected_fragments(&regions,-2,0,5000,&||false,&admit,
        &mut |_|panic!("all edges are to the right"),&mut |_|None),Some(0.));
}

#[test]
fn incremental_owner_matches_full_scan_and_tracks_every_winding_update() {
    use kurbo::Shape;
    let paths=[kurbo::Rect::new(0.1,0.1,0.9,0.9).to_path(0.1),
        kurbo::Rect::new(0.,0.,0.6,1.).to_path(0.1),
        kurbo::Rect::new(0.3,0.,0.8,1.).to_path(0.1),
        kurbo::Rect::new(0.5,0.2,1.,0.7).to_path(0.1)];
    let regions:Vec<_>=paths.iter().map(|p|(p,Fill::NonZero)).collect();
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_row(&regions,0,1_000_000,&||false,&admit).unwrap();
    let choose=|w:&[i32]|if w[0]!=0 {(1..w.len()).rev().find(|&i|w[i]!=0)} else {None};
    let mut expected=Vec::new();
    let expected_area=row.pixel_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,
        &mut |w|Some(choose(w)),&mut |f| {expected.push((f.area,f.centroid,f.owner));Some(())});
    let mut cached=[0;4];let mut updates=0;let mut resets=0;let mut actual=Vec::new();
    let area=row.pixel_incremental_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,4,
        &mut |w,changed| {
            if let Some(i)=changed {updates+=1;cached[i]=w[i];} else {resets+=1;cached.copy_from_slice(w);}
            assert_eq!(&cached,w,"no winding update may be lost, including coincident edges");
            Some(choose(&cached))
        },&mut |f| {actual.push((f.area,f.centroid,f.owner));Some(())});
    assert_eq!(area,expected_area);assert_eq!(actual,expected);assert!(updates>0 && resets>0);
    assert!(row.pixel_incremental_selected_fragments(&regions,0,0,2,&||false,&admit,4,
        &mut |_,_|Some(None),&mut |_|Some(())).is_none());
    assert!(row.pixel_incremental_selected_fragments(&regions,0,0,1_000_000,&||true,&admit,4,
        &mut |_,_|Some(None),&mut |_|Some(())).is_none());
    assert!(row.pixel_incremental_selected_fragments(&regions,0,0,1_000_000,&||false,&|_|None,4,
        &mut |_,_|Some(None),&mut |_|Some(())).is_none());
    assert!(row.pixel_incremental_selected_fragments(&regions,0,0,1_000_000,&||false,&admit,4,
        &mut |_,_|None,&mut |_|Some(())).is_none());
}

#[test]
fn shared_polygon_budget_spans_preparation_and_subrectangles_without_reset() {
    use kurbo::Shape;
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let regions=[(&path,Fill::NonZero)];
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let run=|remaining:&mut usize|->Option<f64> {
        let row=admitted_selected_polygon_row_budget(&regions,0,remaining,&||false,&admit)?;
        let mut area=0.;
        for rect in [kurbo::Rect::new(0.,0.,0.5,1.),kurbo::Rect::new(0.5,0.,1.,1.)] {
            let before=*remaining;
            area+=row.pixel_incremental_selected_rect_fragments_budget(&regions,0,0,rect,
                remaining,&||false,&admit,1,&mut |w,_|Some((w[0]!=0).then_some(0)),
                &mut |_|Some(()))?;
            assert!(*remaining<before,"each tile consumes the existing allowance");
        }
        Some(area)
    };
    let mut remaining=100_000;
    assert_eq!(run(&mut remaining),Some(1.));
    let used=100_000-remaining;
    let mut exact=used;
    assert_eq!(run(&mut exact),Some(1.));assert_eq!(exact,0);
    let mut short=used-1;
    assert_eq!(run(&mut short),None);assert_eq!(short,0);
    assert!(admitted_selected_polygon_row_budget(&regions,0,&mut short,&||false,&admit).is_none());
    assert_eq!(short,0,"exhaustion cannot resurrect credits");
    let mut cancelled=100_000;
    assert!(admitted_selected_polygon_row_budget(&regions,0,&mut cancelled,&||true,&admit).is_none());
    assert_eq!(cancelled,100_000);
}

#[test]
fn narrow_preparation_preserves_fragments_and_excludes_other_vertical_domains() {
    use kurbo::Shape;
    let paint=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let outside=kurbo::Rect::new(0.2,0.75,0.8,0.95).to_path(0.1);
    let mut regions=vec![(&outside,Fill::NonZero);300];regions.push((&paint,Fill::NonZero));
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let mut full_left=20_000_000;let mut narrow_left=full_left;
    let full=admitted_selected_polygon_row_budget(&regions,0,&mut full_left,&||false,&admit).unwrap();
    let narrow=admitted_selected_polygon_domain_budget(&regions,0,0.,0.5,&mut narrow_left,&||false,&admit).unwrap();
    assert!(narrow_left>full_left);assert!(narrow.edges.len()<full.edges.len());
    let rect=kurbo::Rect::new(0.,0.,1.,0.5);
    let collect=|row:&PolygonRow| {
        let mut fragments=Vec::new();let mut remaining=1_000_000;
        let area=row.pixel_incremental_selected_rect_fragments_budget(&regions,0,0,rect,&mut remaining,
            &||false,&admit,1,&mut |w,_|Some((w[300]!=0).then_some(300)),
            &mut |f| {fragments.push((f.area,f.centroid,f.owner));Some(())});
        (area,fragments)
    };
    assert_eq!(collect(&full),collect(&narrow));assert_eq!(collect(&narrow).0,Some(0.5));
    let mut remaining=1_000_000;
    assert!(narrow.pixel_incremental_selected_rect_fragments_budget(&regions,0,0,
        kurbo::Rect::new(0.,0.,1.,1.),&mut remaining,&||false,&admit,1,
        &mut |_,_|Some(Some(300)),&mut |_|Some(())).is_none());
    assert!(admitted_selected_polygon_domain_budget(&regions,0,0.5,0.,&mut remaining,&||false,&admit).is_none());
}


#[test]
fn pixel_row_right_edge_culling_avoids_irrelevant_active_edge_cap() {
    use kurbo::Shape;
    let unit=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let far:Vec<_>=(0..3000).map(|i|kurbo::Rect::new(2.+i as f64,0.,3.+i as f64,1.).to_path(0.1)).collect();
    let mut regions=vec![(&unit,Fill::NonZero)];regions.extend(far.iter().map(|p|(p,Fill::NonZero)));
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let mut ordinary=1_000_000;
    assert!(admitted_selected_polygon_row_budget(&regions,0,&mut ordinary,&||false,&admit).is_none());
    let mut work=1_000_000;
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut work,&||false,&admit).unwrap();
    let area=row.pixel_selected_fragments(&regions,0,0,work,&||false,&admit,
        &mut |w|Some(if w[0]!=0 {Some(0)} else {None}),&mut |_|Some(())).unwrap();
    assert_eq!(area,1.);
    assert!(row.pixel_selected_fragments(&regions,2,0,work,&||false,&admit,
        &mut |_|Some(None),&mut |_|Some(())).is_none());
}

#[test]
fn left_chain_compression_preserves_nonmonotone_winding_and_fill_parity() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    for reversed in [false,true] {for copies in [1,2] {for fill in [Fill::NonZero,Fill::EvenOdd] {
        let mut points=vec![Point::new(-1.,-1.)];
        for i in 0..6000 {points.push(Point::new(-2.-(i%3) as f64/8.,if i%2==0 {0.25} else {0.75}));}
        points.extend([Point::new(-1.,2.),Point::new(0.5,2.),Point::new(0.5,-1.)]);
        if reversed {points.reverse();}
        let mut path=BezPath::new();
        for _ in 0..copies {
            path.move_to(points[0]);
            for &p in &points[1..] {path.line_to(p);}
            path.close_path();
        }
        let regions=[(&path,fill)];
        let mut ordinary=1_000_000;
        assert!(admitted_selected_polygon_row_budget(&regions,0,&mut ordinary,&||false,&admit).is_none());
        let mut work=1_000_000;
        let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut work,&||false,&admit).unwrap();
        assert!(row.edges.len()<=4*copies);
        let expected=if matches!(fill,Fill::EvenOdd) && copies==2 {0.} else {0.5};
        assert_eq!(row.pixel_area(&regions,0,0,work,&||false,&admit),Some(expected));
        // Compression is certified only for the original pixel and its subsets.
        assert!(row.pixel_area(&regions,-1,0,work,&||false,&admit).is_none());
    }}}
}

#[test]
fn left_chain_prepare_refusal_and_every_cancellation_release_credit() {
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));let calls=Cell::new(0);let deny=Cell::new(usize::MAX);
    let admit=|bytes| {
        calls.set(calls.get()+1);
        if calls.get()==deny.get() {return None;}
        used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
    };
    let mut path=BezPath::new();path.move_to((-1.,-1.));
    for i in 0..12 {path.line_to((-2.,if i%2==0 {0.25} else {0.75}));}
    path.line_to((-1.,2.));path.line_to((0.5,2.));path.line_to((0.5,-1.));path.close_path();
    let regions=[(&path,Fill::NonZero)];
    let run=|cancelled:&dyn Fn()->bool| {
        let mut work=1_000_000;
        admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut work,cancelled,&admit)
    };
    let polls=Cell::new(0);
    drop(run(&||{polls.set(polls.get()+1);false}).unwrap());
    let checkpoints=polls.get();let allocations=calls.get();
    assert_eq!(used.load(Ordering::Relaxed),0);
    for target in 1..=allocations {
        calls.set(0);deny.set(target);
        assert!(run(&||false).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    deny.set(usize::MAX);
    for target in 1..=checkpoints {
        polls.set(0);
        assert!(run(&||{polls.set(polls.get()+1);polls.get()>=target}).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    drop(run(&||false).unwrap());
    assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn pixel_owner_triangles_exceed_reusable_row_capacity_with_bounded_storage() {
    let mut triangle=BezPath::new();triangle.move_to((0.,0.25));
    triangle.line_to((1.,0.5));triangle.line_to((0.2,0.75));triangle.close_path();
    let regions=vec![(&triangle,Fill::NonZero);1500];
    let admit=|_:usize|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    assert!(admitted_selected_polygon_row_budget(&regions,0,&mut 40_000_000,&||false,&admit).is_none());
    let mut work=40_000_000;
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut work,&||false,&admit).unwrap();
    assert_eq!(row.edges.len(),4500);
    let area=row.pixel_incremental_selected_fragments(&regions,0,0,work,&||false,&admit,1,
        &mut |winding,_|Some((winding[0]!=0).then_some(0)),&mut |_|Some(())).unwrap();
    assert!((area-0.225).abs()<1e-12);
    assert!(admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 1,&||false,&admit).is_none());
    assert!(admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 40_000_000,&||true,&admit).is_none());
    assert!(admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 40_000_000,&||false,&|_|None).is_none());
}

#[test]
fn shared_fragment_work_debits_retries_without_refreshing_allowance() {
    let mut triangle=BezPath::new();triangle.move_to((0.,0.));
    triangle.line_to((1.,0.));triangle.line_to((0.,1.));triangle.close_path();
    let regions=[(&triangle,Fill::NonZero)];
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 100000,&||false,&admit).unwrap();
    let run=|remaining:&mut usize,cancelled:&dyn Fn()->bool| {
        row.pixel_incremental_selected_fragments_budget(&regions,0,0,remaining,cancelled,&admit,1,
            &mut |w,_|Some((w[0]!=0).then_some(0)),&mut |_|Some(()))
    };
    let mut remaining=100000;
    assert_eq!(run(&mut remaining,&||false),Some(0.5));
    let used=100000-remaining;assert!(used>0);
    let mut exact=used;assert_eq!(run(&mut exact,&||false),Some(0.5));assert_eq!(exact,0);
    let mut shared=used*2-1;
    assert_eq!(run(&mut shared,&||false),Some(0.5));
    assert!(run(&mut shared,&||false).is_none());assert_eq!(shared,0);
    let mut cancelled=used;
    assert!(run(&mut cancelled,&||true).is_none());assert_eq!(cancelled,used);
    assert_eq!(run(&mut cancelled,&||false),Some(0.5));assert_eq!(cancelled,0);
}

#[test]
fn sparse_y_bands_preserve_area_and_affine_integrals_with_bounded_work() {
    let triangle=|y:f64,height:f64| {
        let mut p=BezPath::new();p.move_to((0.,y));p.line_to((1.,y));p.line_to((0.,y+height));p.close_path();p
    };
    let mut paths=vec![triangle(0.,1.)];
    for i in 0..1024 {paths.push(triangle(i as f64/1024.,1./2048.));}
    let regions=paths.iter().map(|p|(p,Fill::NonZero)).collect::<Vec<_>>();
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let mut remaining=40_000_000;
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut remaining,&||false,&admit).expect("prepare sparse row");
    let mut integral=[0.;3];
    let area=row.pixel_incremental_selected_fragments_budget(&regions,0,0,&mut remaining,&||false,&admit,1,
        &mut |w,_|Some((w[0]!=0).then_some(0)),&mut |fragment| {
            integral[0]+=fragment.area;
            integral[1]+=fragment.area*fragment.centroid.x;
            integral[2]+=fragment.area*fragment.centroid.y;
            Some(())
        }).expect("sparse Y edges must fit unchanged work allowance");
    assert!((area-0.5).abs()<1e-12);
    for (actual,expected) in integral.into_iter().zip([0.5,1./6.,1./6.]) {
        assert!((actual-expected).abs()<1e-12,"{actual} vs{expected}");
    }
    eprintln!("SPARSE_Y_BANDS_WORK spent={} remaining={remaining}",40_000_000-remaining);
}

#[test]
fn callback_quotas_are_prepaid_across_subrectangles_without_refund_or_budget_reset() {
    use kurbo::Shape;
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let regions=[(&path,Fill::NonZero)];
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let run=|remaining:&mut usize,mode:u8| {
        let row=admitted_selected_polygon_row_budget(&regions,0,remaining,&||false,&admit)?;
        let mut area=0.;let mut fragments=Vec::new();
        for rect in [kurbo::Rect::new(0.,0.,0.5,1.),kurbo::Rect::new(0.5,0.,1.,1.)] {
            let mut collect=|f:PixelFragment| {
                fragments.push((f.vertices.map(|p|[p.x.to_bits(),p.y.to_bits()]),
                    [f.centroid.x.to_bits(),f.centroid.y.to_bits()],f.area.to_bits(),f.owner));Some(())
            };
            if mode==0 {
                area+=row.pixel_incremental_selected_rect_fragments_budget(&regions,0,0,rect,remaining,
                    &||false,&admit,1,&mut |w,_|Some((w[0]!=0).then_some(0)),&mut collect)?;
            } else {
                area+=row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,remaining,
                    &||false,&admit,1,19,&mut |w,_|Some((w[0]!=0).then_some(0)),
                    &mut |f,local| {assert_eq!(*local,19);if mode==2 {*local=local.checked_sub(19)?;}collect(f)})?;
            }
        }
        Some((area,fragments))
    };
    let mut old=100_000;let expected=run(&mut old,0).unwrap();
    let mut full=100_000;let actual=run(&mut full,2).unwrap();
    assert_eq!(actual,expected);assert_eq!(actual.0,1.);assert_eq!(actual.1.len(),2);
    assert_eq!(old-full,2*19);
    let mut unused=100_000;assert_eq!(run(&mut unused,1),Some(expected.clone()));assert_eq!(unused,full);
    let used=100_000-full;let mut exact=used;
    assert_eq!(run(&mut exact,2),Some(expected));assert_eq!(exact,0);
    let mut short=used-1;assert!(run(&mut short,2).is_none());assert_eq!(short,0);
}

#[test]
fn callback_quota_refusal_and_cancellation_release_scratch_and_never_mint_work() {
    use kurbo::Shape;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self) {self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    let admit=|bytes| {used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let regions=[(&path,Fill::NonZero)];let rect=kurbo::Rect::new(0.,0.,1.,1.);
    {
        let row=admitted_selected_polygon_row_budget(&regions,0,&mut 100_000,&||false,&admit).unwrap();
        let retained=used.load(Ordering::Relaxed);assert!(retained>0);
        let run=|visit:&mut dyn FnMut(PixelFragment,&mut usize)->Option<()>,cancelled:&dyn Fn()->bool| {
            let mut left=100_000;
            let result=row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,&mut left,
                cancelled,&admit,1,19,&mut |w,_|Some((w[0]!=0).then_some(0)),visit);
            (result,left)
        };
        for mut callback in [
            (|_:PixelFragment,q:&mut usize| {*q=q.checked_sub(20)?;Some(())}) as fn(PixelFragment,&mut usize)->Option<()>,
            |_:PixelFragment,q:&mut usize| {*q+=1;Some(())},
        ] {
            let (result,left)=run(&mut callback,&||false);assert!(result.is_none());assert!(left<100_000);
            assert_eq!(used.load(Ordering::Relaxed),retained);
        }
        let cancelled=std::cell::Cell::new(false);
        let (result,left)=run(&mut |_,q| {*q=q.checked_sub(1)?;cancelled.set(true);Some(())},&||cancelled.get());
        assert!(result.is_none());assert!(left<100_000);assert_eq!(used.load(Ordering::Relaxed),retained);
        let (result,left)=run(&mut |_,_|panic!("pre-cancelled callback"),&||true);
        assert!(result.is_none());assert_eq!(left,100_000);
        for quota in [0,1_000_001] {
            let mut left=100_000;
            assert!(row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,&mut left,
                &||false,&admit,1,quota,&mut |_,_|Some(Some(0)),&mut |_,_|panic!("invalid quota")).is_none());
            assert_eq!(left,100_000);
        }
        let mut left=100_000;
        assert!(row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,&mut left,
            &||false,&|_|None,1,19,&mut |_,_|Some(Some(0)),&mut |_,_|panic!("refused scratch admission")).is_none());
        assert_eq!(used.load(Ordering::Relaxed),retained);
    }
    assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn tile_y_edge_filter_preserves_cuts_order_and_refuses_partial_storage() {
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    let admit=|n|{used.fetch_add(n,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),n)) as Box<dyn std::any::Any+Send+Sync>)};
    let mut edges=Vec::new();
    for y in -4..=8 {for reverse in [false,true] {
        let a=Point::new(-0.25,y as f64/4.);let b=Point::new(0.75,(y+1) as f64/4.);
        edges.push(Edge {a:if reverse {b}else{a},b:if reverse {a}else{b},region:edges.len()});
        edges.push(Edge {a,b:Point::new(0.75,a.y),region:edges.len()});
    }}
    for (top,bottom) in [(0.,1.),(0.,0.25),(0.25,0.5),(0.5,0.75),(0.75,1.)] {
        let mut work=Work {left:100000,cancelled:&||false};
        let filtered=admitted_tile_y_edges(&edges,top,bottom,&mut work,&admit).unwrap();
        let expected:Vec<_>=edges.iter().filter(|e|e.low()<=bottom && e.high()>=top).map(|e|e.region).collect();
        assert_eq!(filtered.edges.iter().map(|e|e.region).collect::<Vec<_>>(),expected);
        let mut before=Vec::new();let mut after=Vec::new();
        visit_cuts(&edges,&[],(0.,1.,top,bottom),&mut work,&mut |v|{before.push(v.to_bits());Some(())}).unwrap();
        visit_cuts(&filtered.edges,&[],(0.,1.,top,bottom),&mut work,&mut |v|{after.push(v.to_bits());Some(())}).unwrap();
        assert_eq!(before,after);drop(filtered);assert_eq!(used.load(Ordering::Relaxed),0);
    }
    let required=2*edges.len()+2;
    for allowance in 0..required {
        assert!(admitted_tile_y_edges(&edges,0.,0.25,&mut Work {left:allowance,cancelled:&||false},&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    for stop in 1..=required {
        let polls=Cell::new(0);let cancel=||{polls.set(polls.get()+1);polls.get()>=stop};
        assert!(admitted_tile_y_edges(&edges,0.,0.25,&mut Work {left:100000,cancelled:&cancel},&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    assert!(admitted_tile_y_edges(&edges,0.,0.25,&mut Work {left:100000,cancelled:&||false},&|_|None).is_none());
}

// Bounds the exact centroid of the kernel's nonnegative-width repaired trapezoid,
// including the kernel's
// floating evaluation. This does not enclose edge intersection/clipping error.
#[derive(Clone, Copy)]
struct FragmentInterval {
    lo: f64,
    hi: f64,
}
impl FragmentInterval {
    fn exact(v: f64) -> Option<Self> {
        v.is_finite().then_some(Self { lo: v, hi: v })
    }
    fn op(self, rhs: Self, kind: u8, work: &mut Work<'_>) -> Option<Self> {
        work.step(1)?;
        let (lo, hi) = match kind {
            0 => (self.lo + rhs.lo, self.hi + rhs.hi),
            1 => (self.lo - rhs.hi, self.hi - rhs.lo),
            2 | 3 => {
                if kind == 3 && rhs.lo <= 0. && rhs.hi >= 0. {
                    return None;
                }
                let values = if kind == 2 {
                    [
                        self.lo * rhs.lo,
                        self.lo * rhs.hi,
                        self.hi * rhs.lo,
                        self.hi * rhs.hi,
                    ]
                } else {
                    [
                        self.lo / rhs.lo,
                        self.lo / rhs.hi,
                        self.hi / rhs.lo,
                        self.hi / rhs.hi,
                    ]
                };
                (
                    values.into_iter().fold(f64::INFINITY, f64::min),
                    values.into_iter().fold(f64::NEG_INFINITY, f64::max),
                )
            }
            _ => return None,
        };
        if !lo.is_finite() || !hi.is_finite() {
            return None;
        }
        let out = Self {
            lo: lo.next_down(),
            hi: hi.next_up(),
        };
        (out.lo.is_finite() && out.hi.is_finite()).then_some(out)
    }
}
fn trapezoid_centroid_enclosure(
    vertices: [Point; 4],
    left: f64,
    work: &mut Work<'_>,
) -> Option<[[f64; 2]; 2]> {
    work.step(1)?;
    for p in vertices {
        work.step(1)?;
        if !p.x.is_finite() || !p.y.is_finite() {
            return None;
        }
    }
    let [a0, a1, b1, b0] = vertices;
    if !left.is_finite() || a0.y != a1.y || b0.y != b1.y || a0.y >= b0.y || left > a0.x.min(b0.x) {
        return None;
    }
    let exact = FragmentInterval::exact;
    let mut op = |a: FragmentInterval, b: FragmentInterval, k| a.op(b, k, work);
    let mut wa = op(exact(a1.x)?, exact(a0.x)?, 1)?;
    let mut wb = op(exact(b1.x)?, exact(b0.x)?, 1)?;
    // The kernel explicitly clamps inverted endpoint widths to zero.
    wa.lo = wa.lo.max(0.);
    wa.hi = wa.hi.max(0.);
    wb.lo = wb.lo.max(0.);
    wb.hi = wb.hi.max(0.);
    let total = op(wa, wb, 0)?;
    if total.lo <= 0. {
        return None;
    }
    let u = op(wa, total, 3)?;
    let v = op(exact(1.)?, u, 1)?;
    let ma0 = op(exact(a0.x)?, exact(left)?, 1)?;
    let ma1 = op(wa, exact(0.5)?, 2)?;
    let ma = op(ma0, ma1, 0)?;
    let mb0 = op(exact(b0.x)?, exact(left)?, 1)?;
    let mb1 = op(wb, exact(0.5)?, 2)?;
    let mb = op(mb0, mb1, 0)?;
    let twice_u = op(exact(2.)?, u, 2)?;
    let twice_v = op(exact(2.)?, v, 2)?;
    let top_weight = op(twice_u, v, 0)?;
    let bottom_weight = op(u, twice_v, 0)?;
    let top_x = op(top_weight, ma, 2)?;
    let bottom_x = op(bottom_weight, mb, 2)?;
    let x_sum = op(top_x, bottom_x, 0)?;
    let x_offset = op(x_sum, exact(3.)?, 3)?;
    let x = op(exact(left)?, x_offset, 0)?;
    let height = op(exact(b0.y)?, exact(a0.y)?, 1)?;
    let y_weighted = op(height, bottom_weight, 2)?;
    let y_offset = op(y_weighted, exact(3.)?, 3)?;
    let y = op(exact(a0.y)?, y_offset, 0)?;
    Some([[x.lo, x.hi], [y.lo, y.hi]])
}

#[test]
fn trapezoid_centroid_enclosure_contains_exact_rational_and_kernel_centroids() {
    // Positive normal f64 endpoints compared to the rational without rounding
    // the rational to f64. These integer test domains fit checked i128 products.
    fn compare(value: f64, n: i128, d: i128) -> std::cmp::Ordering {
        assert!(value.is_normal() && value > 0. && n > 0 && d > 0);
        let bits = value.to_bits();
        let m = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
        let e = ((bits >> 52) & 2047) as i32 - 1023 - 52;
        if e >= 0 {
            (m.checked_shl(e as u32).unwrap() * d).cmp(&n)
        } else {
            (m * d).cmp(&n.checked_shl((-e) as u32).unwrap())
        }
    }
    for base in [100_i128, 1_000_000] {
        for wa in -2_i128..8 {
            for wb in -2_i128..8 {
                if wa.max(0) + wb.max(0) == 0 {
                    continue;
                }
                for ax in 0_i128..4 {
                    for bx in 0_i128..4 {
                        for height in 1_i128..4 {
                            let xa = base + ax;
                            let xb = base + bx;
                            let a = 10_i128;
                            let b = a + height;
                            let vertices = [
                                Point::new(xa as f64, a as f64),
                                Point::new((xa + wa) as f64, a as f64),
                                Point::new((xb + wb) as f64, b as f64),
                                Point::new(xb as f64, b as f64),
                            ];
                            let bounds = trapezoid_centroid_enclosure(
                                vertices,
                                base as f64,
                                &mut Work {
                                    left: 1000,
                                    cancelled: &|| false,
                                },
                            )
                            .unwrap();
                            let wa = wa.max(0);
                            let wb = wb.max(0);
                            let n_x = (2 * wa + wb) * (2 * xa + wa) + (wa + 2 * wb) * (2 * xb + wb);
                            let d_x = 6 * (wa + wb);
                            let n_y = a * 3 * (wa + wb) + height * (wa + 2 * wb);
                            let d_y = 3 * (wa + wb);
                            for (bound, n, d) in [(bounds[0], n_x, d_x), (bounds[1], n_y, d_y)] {
                                assert!(!compare(bound[0], n, d).is_gt());
                                assert!(!compare(bound[1], n, d).is_lt());
                            }
                            let u = wa as f64 / (wa + wb) as f64;
                            let v = 1. - u;
                            let ma = (xa - base) as f64 + wa as f64 * 0.5;
                            let mb = (xb - base) as f64 + wb as f64 * 0.5;
                            let actual = [
                                base as f64 + ((2. * u + v) * ma + (u + 2. * v) * mb) / 3.,
                                a as f64 + height as f64 * (u + 2. * v) / 3.,
                            ];
                            for axis in 0..2 {
                                assert!(
                                    bounds[axis][0] <= actual[axis]
                                        && actual[axis] <= bounds[axis][1]
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn trapezoid_centroid_enclosure_refuses_invalid_short_work_and_every_cancel() {
    let vertices = [
        Point::new(10., 10.),
        Point::new(11., 10.),
        Point::new(12., 11.),
        Point::new(10., 11.),
    ];
    let mut work = Work {
        left: 1000,
        cancelled: &|| false,
    };
    trapezoid_centroid_enclosure(vertices, 10., &mut work).unwrap();
    let used = 1000 - work.left;
    for allowance in 0..used {
        assert!(
            trapezoid_centroid_enclosure(
                vertices,
                10.,
                &mut Work {
                    left: allowance,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
    for stop in 1..=used {
        let polls = std::cell::Cell::new(0);
        let cancel = || {
            polls.set(polls.get() + 1);
            polls.get() >= stop
        };
        assert!(
            trapezoid_centroid_enclosure(
                vertices,
                10.,
                &mut Work {
                    left: 1000,
                    cancelled: &cancel
                }
            )
            .is_none()
        );
    }
    for i in 0..4 {
        let mut invalid = vertices;
        invalid[i].x = f64::NAN;
        assert!(
            trapezoid_centroid_enclosure(
                invalid,
                10.,
                &mut Work {
                    left: 1000,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
    for invalid in [
        [Point::ZERO; 4],
        [vertices[1], vertices[0], vertices[3], vertices[2]],
    ] {
        assert!(
            trapezoid_centroid_enclosure(
                invalid,
                10.,
                &mut Work {
                    left: 1000,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
}

// Encloses ideal affine edge interpolation and the kernel's computed X. This
// does not prove that the sweep selected the correct edge or crossing order.
fn edge_slope_enclosure(edge: Edge, work: &mut Work<'_>) -> Option<FragmentInterval> {
    work.step(1)?;
    let (a, b) = if edge.a.y <= edge.b.y {
        (edge.a, edge.b)
    } else {
        (edge.b, edge.a)
    };
    let ax = FragmentInterval::exact(a.x)?;
    let ay = FragmentInterval::exact(a.y)?;
    let bx = FragmentInterval::exact(b.x)?;
    let by = FragmentInterval::exact(b.y)?;
    if a.y == b.y {
        return FragmentInterval::exact(0.);
    }
    let dx = bx.op(ax, 1, work)?;
    let dy = by.op(ay, 1, work)?;
    dx.op(dy, 3, work)
}
fn edge_x_enclosure(edge: Edge, y: f64, work: &mut Work<'_>) -> Option<[f64; 2]> {
    work.step(1)?;
    let a = if edge.a.y <= edge.b.y { edge.a } else { edge.b };
    let slope = edge_slope_enclosure(edge, work)?;
    let dy = FragmentInterval::exact(y)?.op(FragmentInterval::exact(a.y)?, 1, work)?;
    let shift = dy.op(slope, 2, work)?;
    let value = FragmentInterval::exact(a.x)?.op(shift, 0, work)?;
    Some([value.lo, value.hi])
}
fn crossing_y_enclosure(
    edge: Edge,
    other: Edge,
    anchor: f64,
    work: &mut Work<'_>,
) -> Option<[f64; 2]> {
    work.step(1)?;
    if edge.a.y == edge.b.y || other.a.y == other.b.y {
        return None;
    }
    let a = edge_slope_enclosure(edge, work)?;
    let b = edge_slope_enclosure(other, work)?;
    let denominator = a.op(b, 1, work)?;
    if denominator.lo <= 0. && denominator.hi >= 0. {
        return None;
    }
    let x = edge_x_enclosure(edge, anchor, work)?;
    let y = edge_x_enclosure(other, anchor, work)?;
    let numerator = FragmentInterval { lo: y[0], hi: y[1] }.op(
        FragmentInterval { lo: x[0], hi: x[1] },
        1,
        work,
    )?;
    let offset = numerator.op(denominator, 3, work)?;
    let value = FragmentInterval::exact(anchor)?.op(offset, 0, work)?;
    Some([value.lo, value.hi])
}

#[test]
fn edge_x_enclosure_contains_exact_rational_and_both_traversal_orders() {
    fn compare(value: f64, n: i128, d: i128) -> std::cmp::Ordering {
        if value < 0. {
            return compare(-value, -n, d).reverse();
        }
        if n <= 0 {
            return if value == 0. {
                0_i128.cmp(&n)
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if value == 0. {
            return std::cmp::Ordering::Less;
        }
        assert!(value.is_normal());
        let bits = value.to_bits();
        let m = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
        let e = ((bits >> 52) & 2047) as i32 - 1023 - 52;
        if e >= 0 {
            (m.checked_shl(e as u32).unwrap() * d).cmp(&n)
        } else {
            (m * d).cmp(&n.checked_shl((-e) as u32).unwrap())
        }
    }
    for base in [-1_000_000_i128, -100, 100, 1_000_000] {
        for dx in -7_i128..8 {
            for height in 1_i128..8 {
                for position in 0_i128..=height {
                    let a = Point::new(base as f64, 10.);
                    let b = Point::new((base + dx) as f64, (10 + height) as f64);
                    let y = (10 + position) as f64;
                    let n = base * height + dx * position;
                    let mut bounds = None;
                    for (a, b) in [(a, b), (b, a)] {
                        let edge = Edge { a, b, region: 0 };
                        let interval = edge_x_enclosure(
                            edge,
                            y,
                            &mut Work {
                                left: 1000,
                                cancelled: &|| false,
                            },
                        )
                        .unwrap();
                        assert!(!compare(interval[0], n, height).is_gt());
                        assert!(!compare(interval[1], n, height).is_lt());
                        assert!(interval[0] <= edge.x(y) && edge.x(y) <= interval[1]);
                        if let Some(previous) = bounds {
                            assert_eq!(interval, previous);
                        } else {
                            bounds = Some(interval);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn edge_x_enclosure_refuses_invalid_short_work_and_every_cancel() {
    let edge = Edge {
        a: Point::new(-1., 2.),
        b: Point::new(3., 5.),
        region: 0,
    };
    let mut work = Work {
        left: 100,
        cancelled: &|| false,
    };
    edge_x_enclosure(edge, 3., &mut work).unwrap();
    let used = 100 - work.left;
    for allowance in 0..used {
        assert!(
            edge_x_enclosure(
                edge,
                3.,
                &mut Work {
                    left: allowance,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
    for stop in 1..=used {
        let polls = std::cell::Cell::new(0);
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() >= stop
        };
        assert!(
            edge_x_enclosure(
                edge,
                3.,
                &mut Work {
                    left: 100,
                    cancelled: &cancelled
                }
            )
            .is_none()
        );
    }
    assert!(
        edge_x_enclosure(
            edge,
            f64::NAN,
            &mut Work {
                left: 100,
                cancelled: &|| false
            }
        )
        .is_none()
    );
    let horizontal = Edge {
        a: Point::new(2., 3.),
        b: Point::new(4., 3.),
        region: 0,
    };
    let bounds = edge_x_enclosure(
        horizontal,
        3.,
        &mut Work {
            left: 100,
            cancelled: &|| false,
        },
    )
    .unwrap();
    assert!(bounds[0] <= 2. && bounds[1] >= 2.);
}

#[test]
fn crossing_y_enclosure_contains_exact_rational_and_kernel_crossings() {
    fn compare(value: f64, n: i128, d: i128) -> std::cmp::Ordering {
        assert!(value.is_normal() && value > 0. && n > 0 && d > 0);
        let bits = value.to_bits();
        let m = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
        let e = ((bits >> 52) & 2047) as i32 - 1023 - 52;
        if e >= 0 {
            (m.checked_shl(e as u32).unwrap() * d).cmp(&n)
        } else {
            (m * d).cmp(&n.checked_shl((-e) as u32).unwrap())
        }
    }
    for base in [-1_000_000_i128, -100, 100, 1_000_000] {
        for height in 1_i128..8 {
            for dx in 1_i128..6 {
                for other_dx in -5_i128..0 {
                    for offset in 1_i128..dx - other_dx {
                        let a = Point::new(base as f64, 10.);
                        let b = Point::new((base + dx) as f64, (10 + height) as f64);
                        let c = Point::new((base + offset) as f64, 10.);
                        let d = Point::new((base + offset + other_dx) as f64, (10 + height) as f64);
                        let n = 10 * (dx - other_dx) + offset * height;
                        let denominator = dx - other_dx;
                        for (a, b) in [(a, b), (b, a)] {
                            for (c, d) in [(c, d), (d, c)] {
                                let edge = Edge { a, b, region: 0 };
                                let other = Edge {
                                    a: c,
                                    b: d,
                                    region: 1,
                                };
                                let interval = crossing_y_enclosure(
                                    edge,
                                    other,
                                    10.,
                                    &mut Work {
                                        left: 1000,
                                        cancelled: &|| false,
                                    },
                                )
                                .unwrap();
                                assert!(!compare(interval[0], n, denominator).is_gt());
                                assert!(!compare(interval[1], n, denominator).is_lt());
                                let actual = 10.
                                    + (other.x(10.) - edge.x(10.)) / (edge.slope() - other.slope());
                                assert!(interval[0] <= actual && actual <= interval[1]);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn crossing_y_enclosure_refuses_parallel_uncertain_short_work_and_every_cancel() {
    let edge = Edge {
        a: Point::new(0., 0.),
        b: Point::new(2., 2.),
        region: 0,
    };
    let other = Edge {
        a: Point::new(2., 0.),
        b: Point::new(0., 2.),
        region: 1,
    };
    let mut work = Work {
        left: 1000,
        cancelled: &|| false,
    };
    crossing_y_enclosure(edge, other, 0., &mut work).unwrap();
    let used = 1000 - work.left;
    for allowance in 0..used {
        assert!(
            crossing_y_enclosure(
                edge,
                other,
                0.,
                &mut Work {
                    left: allowance,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
    for stop in 1..=used {
        let polls = std::cell::Cell::new(0);
        let cancel = || {
            polls.set(polls.get() + 1);
            polls.get() >= stop
        };
        assert!(
            crossing_y_enclosure(
                edge,
                other,
                0.,
                &mut Work {
                    left: 1000,
                    cancelled: &cancel
                }
            )
            .is_none()
        );
    }
    assert!(
        crossing_y_enclosure(
            edge,
            edge,
            0.,
            &mut Work {
                left: 1000,
                cancelled: &|| false
            }
        )
        .is_none()
    );
    let near = Edge {
        a: Point::new(0.125, 0.),
        b: Point::new(f64::from_bits(2.125f64.to_bits() + 1), 2.),
        region: 1,
    };
    assert!(
        crossing_y_enclosure(
            edge,
            near,
            0.,
            &mut Work {
                left: 1000,
                cancelled: &|| false
            }
        )
        .is_none()
    );
    assert!(
        crossing_y_enclosure(
            edge,
            other,
            f64::NAN,
            &mut Work {
                left: 1000,
                cancelled: &|| false
            }
        )
        .is_none()
    );
}

#[test]
fn exact_shared_vertex_skips_rounded_ghost_but_keeps_distinct_interior_crossing() {
    let common = Point::new(138.67889560216426, 34.414455734213455);
    let a = Point::new(143.06237100075353, 32.985484273566186);
    let b = Point::new(146.61264899861916, 32.369134202116584);
    let collect = |edges: &[Edge], skip| {
        let mut found = Vec::new();
        visit_crossings_with_shared_vertex_policy(
            edges,
            0.,
            100.,
            &mut Work {
                left: 10000,
                cancelled: &|| false,
            },
            &mut |p| {
                found.push(p);
                Some(())
            },
            skip,
            false,
        )
        .unwrap();
        found
    };
    for (a, common_a) in [(a, common), (common, a)] {
        for (b, common_b) in [(b, common), (common, b)] {
            let edges = [
                Edge {
                    a,
                    b: common_a,
                    region: 0,
                },
                Edge {
                    a: b,
                    b: common_b,
                    region: 1,
                },
            ];
            let legacy = collect(&edges, false);
            assert_eq!(legacy.len(), 1);
            assert!(legacy[0].1 < common.y && common.y - legacy[0].1 < 1e-12);
            assert!(collect(&edges, true).is_empty());
        }
    }
    let edges = [
        Edge {
            a: Point::ZERO,
            b: Point::new(2., 2.),
            region: 0,
        },
        Edge {
            a: Point::new(2., 0.),
            b: Point::new(0., 2.),
            region: 1,
        },
    ];
    assert_eq!(collect(&edges, true), vec![(1., 1.)]);
    assert_eq!(collect(&edges, true), collect(&edges, false));
}

fn orientation_interval(a: Point, b: Point, c: Point, work: &mut Work<'_>) -> Option<[f64; 2]> {
    work.step(1)?;
    let e = FragmentInterval::exact;
    let bx = e(b.x)?.op(e(a.x)?, 1, work)?;
    let by = e(b.y)?.op(e(a.y)?, 1, work)?;
    let cx = e(c.x)?.op(e(a.x)?, 1, work)?;
    let cy = e(c.y)?.op(e(a.y)?, 1, work)?;
    let left = bx.op(cy, 2, work)?;
    let right = by.op(cx, 2, work)?;
    let out = left.op(right, 1, work)?;
    Some([out.lo, out.hi])
}
fn orientation_sign(a: Point, b: Point, c: Point, work: &mut Work<'_>) -> Option<i8> {
    let [lo, hi] = orientation_interval(a, b, c, work)?;
    if lo > 0. {
        return Some(1);
    }
    if hi < 0. {
        return Some(-1);
    }
    work.step(1)?;
    // Exact residual products must remain normal. These conservative operand
    // limits keep all 106-bit product tails above2^-1006 and sums below2^905.
    let small = 2f64.powi(-450);
    let large = 2f64.powi(450);
    let admissible = |x: f64| x.is_finite() && (x == 0. || (x.abs() >= small && x.abs() <= large));
    if [a.x, a.y, b.x, b.y, c.x, c.y]
        .iter()
        .any(|v| !admissible(*v))
    {
        return None;
    }
    let difference = |x: f64, y: f64| {
        let high = x - y;
        let virtual_y = x - high;
        let virtual_x = high + virtual_y;
        [high, (x - virtual_x) + (virtual_y - y)]
    };
    let bx = difference(b.x, a.x);
    let by = difference(b.y, a.y);
    let cx = difference(c.x, a.x);
    let cy = difference(c.y, a.y);
    if bx
        .into_iter()
        .chain(by)
        .chain(cx)
        .chain(cy)
        .any(|v| !admissible(v))
    {
        return None;
    }
    let mut expansion = [0f64; 32];
    let mut length = 0usize;
    let mut add = |value: f64, work: &mut Work<'_>| -> Option<()> {
        let mut next = [0f64; 32];
        let mut count = 0;
        let mut sum = value;
        for &component in &expansion[..length] {
            work.step(1)?;
            let high = sum + component;
            let virtual_component = high - sum;
            let low = (sum - (high - virtual_component)) + (component - virtual_component);
            if low != 0. {
                if count == next.len() {
                    return None;
                }
                next[count] = low;
                count += 1;
            }
            sum = high;
        }
        if sum != 0. {
            if count == next.len() {
                return None;
            }
            next[count] = sum;
            count += 1;
        }
        expansion = next;
        length = count;
        Some(())
    };
    for (left, right, sign) in [(bx, cy, 1.), (by, cx, -1.)] {
        for x in left {
            for y in right {
                work.step(1)?;
                let product = x * y;
                let tail = x.mul_add(y, -product);
                if !product.is_finite() || !tail.is_finite() {
                    return None;
                }
                add(sign * tail, work)?;
                add(sign * product, work)?;
            }
        }
    }
    work.step(1)?;
    Some(if length == 0 {
        0
    } else if expansion[length - 1] > 0. {
        1
    } else {
        -1
    })
}
// A proper interior crossing requires strict opposite signs on both lines.
// Zero is an exact endpoint/collinear contact, never a distinct interior cut.
fn proper_segment_crossing(a: Edge, b: Edge, work: &mut Work<'_>) -> Option<bool> {
    let c = orientation_sign(a.a, a.b, b.a, work)?;
    if c == 0 {
        return Some(false);
    }
    let d = orientation_sign(a.a, a.b, b.b, work)?;
    if d == 0 || c == d {
        return Some(false);
    }
    let e = orientation_sign(b.a, b.b, a.a, work)?;
    if e == 0 {
        return Some(false);
    }
    let f = orientation_sign(b.a, b.b, a.b, work)?;
    Some(f != 0 && e != f)
}

#[test]
fn adaptive_segment_crossing_matches_exact_parametric_integer_oracle() {
    let cross = |a: (i128, i128), b: (i128, i128)| a.0 * b.1 - a.1 * b.0;
    for base in [-1_000_000., 0., 1_000_000.] {
        for scale in [2f64.powi(-30), 1., 2f64.powi(30)] {
            let points: Vec<_> = (0_i128..3)
                .flat_map(|y| (0_i128..3).map(move |x| (x, y)))
                .collect();
            for &a in &points {
                for &b in &points {
                    for &c in &points {
                        for &d in &points {
                            let ab = (b.0 - a.0, b.1 - a.1);
                            let cd = (d.0 - c.0, d.1 - c.1);
                            let ac = (c.0 - a.0, c.1 - a.1);
                            let denominator = cross(ab, cd);
                            let t = cross(ac, cd);
                            let u = cross(ac, ab);
                            let interior = |n: i128| {
                                if denominator > 0 {
                                    n > 0 && n < denominator
                                } else {
                                    n < 0 && n > denominator
                                }
                            };
                            let expected = denominator != 0 && interior(t) && interior(u);
                            let p = |v: (i128, i128)| {
                                Point::new(base + v.0 as f64 * scale, base + v.1 as f64 * scale)
                            };
                            let actual = proper_segment_crossing(
                                Edge {
                                    a: p(a),
                                    b: p(b),
                                    region: 0,
                                },
                                Edge {
                                    a: p(c),
                                    b: p(d),
                                    region: 1,
                                },
                                &mut Work {
                                    left: 10000,
                                    cancelled: &|| false,
                                },
                            )
                            .unwrap();
                            assert_eq!(
                                actual, expected,
                                "{a:?}{b:?}{c:?}{d:?} base{base} scale{scale}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn adaptive_orientation_preserves_near_collinearity_and_refuses_short_work_cancel() {
    for i in 0_i128..128 {
        let bx = (1_i128 << 50) + i * 7919;
        let by = (1_i128 << 50) + i * 3571 + 1;
        for (cx, cy) in [(2 * bx + 1, 2 * by), (2 * bx, 2 * by + 1), (2 * bx, 2 * by)] {
            let p =
                |x: i128, y: i128| Point::new(x as f64 * 2f64.powi(-40), y as f64 * 2f64.powi(-40));
            let n = bx * cy - by * cx;
            let expected = if n > 0 {
                1
            } else if n < 0 {
                -1
            } else {
                0
            };
            assert_eq!(
                orientation_sign(
                    Point::ZERO,
                    p(bx, by),
                    p(cx, cy),
                    &mut Work {
                        left: 10000,
                        cancelled: &|| false
                    }
                ),
                Some(expected)
            );
        }
    }
    let a = Point::ZERO;
    let b = Point::new(1., 1.);
    let c = Point::new(2., 2.);
    let mut work = Work {
        left: 10000,
        cancelled: &|| false,
    };
    assert_eq!(orientation_sign(a, b, c, &mut work), Some(0));
    let used = 10000 - work.left;
    for allowance in 0..used {
        assert!(
            orientation_sign(
                a,
                b,
                c,
                &mut Work {
                    left: allowance,
                    cancelled: &|| false
                }
            )
            .is_none()
        );
    }
    for stop in 1..=used {
        let polls = std::cell::Cell::new(0);
        let cancel = || {
            polls.set(polls.get() + 1);
            polls.get() >= stop
        };
        assert!(
            orientation_sign(
                a,
                b,
                c,
                &mut Work {
                    left: 10000,
                    cancelled: &cancel
                }
            )
            .is_none()
        );
    }
    assert!(
        orientation_sign(
            Point::new(f64::NAN, 0.),
            b,
            c,
            &mut Work {
                left: 10000,
                cancelled: &|| false
            }
        )
        .is_none()
    );
}

#[test]
fn tile_region_subset_preserves_every_fragment_and_clears_previous_tile() {
    use kurbo::Shape;
    let mut paths=vec![kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1)];
    for i in 0..80 {
        let top=(i%8) as f64/8.;
        let mut path=kurbo::Rect::new(0.07+(i%3) as f64*0.03,top,0.91,top+0.09375).to_path(0.1);
        if i%6==1 {let repeated=path.clone();path.extend(repeated.elements().iter().copied());}
        paths.push(path);
    }
    let regions:Vec<_>=paths.iter().enumerate().map(|(i,p)|(p,if i%2==0 {Fill::NonZero} else {Fill::EvenOdd})).collect();
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 10_000_000,&||false,&admit).unwrap();
    let run=|sparse:bool| {
        let mut saved=vec![0;regions.len()];let mut fragments=Vec::new();let mut initialized=0;let mut reduced=0;
        let mut selector=|winding:&[i32],changed:Option<usize>,subset:Option<&[usize]>| {
            if let Some(i)=changed {saved[i]=winding[i];}
            else if let Some(indices)=subset {
                assert!(indices.windows(2).all(|p|p[0]<p[1]));
                for (i,&value) in winding.iter().enumerate() {assert!(value==0 || indices.binary_search(&i).is_ok());}
                for &i in indices {saved[i]=winding[i];}reduced+=1;
            } else {saved.copy_from_slice(winding);initialized+=1;}
            assert_eq!(saved,winding);
            Some(saved.iter().enumerate().rfind(|(i,w)|match regions[*i].1 {Fill::NonZero=>**w!=0,Fill::EvenOdd=>**w%2!=0}).map(|(i,_)|i))
        };
        let mut visitor=|f:PixelFragment,_:&mut usize| {
            fragments.push((f.owner,f.area.to_bits(),[f.centroid.x.to_bits(),f.centroid.y.to_bits()],f.vertices.map(|p|[p.x.to_bits(),p.y.to_bits()])));Some(())
        };
        let mut remaining=10_000_000;
        for i in 0..8 {
            let rect=kurbo::Rect::new(0.,i as f64/8.,1.,(i+1) as f64/8.);
            if sparse {row.pixel_rect_fragments_with_subset_callback_budget(&regions,0,0,rect,&mut remaining,&||false,&admit,6,1,&mut selector,&mut visitor).unwrap();}
            else {row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,&mut remaining,&||false,&admit,6,1,&mut |w,c|selector(w,c,None),&mut visitor).unwrap();}
        }
        (fragments,initialized,reduced)
    };
    let full=run(false);let sparse=run(true);assert_eq!(full.0,sparse.0);assert_eq!(sparse.1,8);
    assert!(sparse.2>0);
}

#[test]
fn tile_region_subset_refuses_short_work_cancel_and_storage_without_credit_leak() {
    use kurbo::Shape;
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let mut triangle=BezPath::new();triangle.move_to((0.1,0.1));triangle.line_to((0.9,0.4));triangle.line_to((0.2,0.9));triangle.close_path();
    let regions=[(&path,Fill::NonZero),(&triangle,Fill::EvenOdd)];
    let live=Arc::new(AtomicUsize::new(0));
    let admit=|n| {live.fetch_add(n,Ordering::Relaxed);Some(Box::new(Credit(live.clone(),n)) as Box<dyn std::any::Any+Send+Sync>)};
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 1_000_000,&||false,&admit).unwrap();
    let held=live.load(Ordering::Relaxed);
    let run=|remaining:&mut usize,cancelled:&dyn Fn()->bool,storage:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>| {
        let mut saved=[0;2];
        row.pixel_rect_fragments_with_subset_callback_budget(&regions,0,0,kurbo::Rect::new(0.,0.,1.,1.),remaining,cancelled,storage,6,1,
            &mut |w,c,subset| {
                if let Some(i)=c {saved[i]=w[i];}
                else if let Some(indices)=subset {assert!(indices.windows(2).all(|p|p[0]<p[1]));for &i in indices {saved[i]=w[i];}}
                else {saved.copy_from_slice(w);}
                assert_eq!(saved,w);Some(saved.iter().rposition(|&v|v!=0))
            },&mut |_,_|Some(()))
    };
    let calls=Cell::new(0);let mut work=1_000_000;
    assert!(run(&mut work,&||{calls.set(calls.get()+1);false},&admit).is_some());
    assert_eq!(live.load(Ordering::Relaxed),held);
    for stop in 0..calls.get() {
        let n=Cell::new(0);let mut work=1_000_000;
        assert!(run(&mut work,&||{let old=n.get();n.set(old+1);old>=stop},&admit).is_none());
        assert_eq!(live.load(Ordering::Relaxed),held);
    }
    for limit in 0..(1_000_000-work) {
        let mut left=limit;assert!(run(&mut left,&||false,&admit).is_none());assert!(left<=limit);
        assert_eq!(live.load(Ordering::Relaxed),held);
    }
    assert!(run(&mut 1_000_000,&||false,&|_|None).is_none());assert_eq!(live.load(Ordering::Relaxed),held);
    drop(row);assert_eq!(live.load(Ordering::Relaxed),0);
}

#[test]
fn left_edge_events_preserve_bit_exact_fragments_across_tiles_and_fill_rules() {
    use kurbo::Shape;
    let mut paths=vec![kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1)];
    for i in 0..24 {
        let shift=i as f64/100.;let mut p=BezPath::new();
        p.move_to((-10.,-0.2+shift));p.line_to((0.7,0.1+shift));p.line_to((-3.,0.35+shift));
        p.line_to((0.9,0.6+shift));p.line_to((-8.,0.8+shift));p.line_to((-4.,1.2+shift));p.close_path();
        if i%3==0 {let repeat=p.clone();p.extend(repeat.elements().iter().copied());}
        paths.push(p);
    }
    let regions:Vec<_>=paths.iter().enumerate().map(|(i,p)|(p,if i%2==0 {Fill::NonZero} else {Fill::EvenOdd})).collect();
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 10_000_000,&||false,&admit).unwrap();
    let run=|events| {
        let mut out=Vec::new();let mut work=Work {left:10_000_000,cancelled:&||false};
        for ty in 0..8 {for tx in 0..8 {
            let rect=kurbo::Rect::new(tx as f64/8.,ty as f64/8.,(tx+1) as f64/8.,(ty+1) as f64/8.);
            let area=row.pixel_area_with_work_policy::<false>(&regions,0,0,&mut work,&admit,
                Some(&mut |f:PixelFragment| {out.push((f.owner,f.area.to_bits(),[f.centroid.x.to_bits(),f.centroid.y.to_bits()],f.vertices.map(|p|[p.x.to_bits(),p.y.to_bits()])));Some(())}),
                Some(&mut |w|Some(w.iter().enumerate().rfind(|(i,v)|match regions[*i].1 {Fill::NonZero=>**v!=0,Fill::EvenOdd=>**v%2!=0}).map(|(i,_)|i))),None,Some(rect),0,None,events).unwrap();
            assert!(area.is_finite());
        }}out
    };
    assert_eq!(run(false),run(true));
    // Reversed traversal, exact border contact, and a finite huge slope.
    for (a,b) in [(Point::new(-1.,0.),Point::new(0.,1.)),(Point::new(-1e200,0.),Point::new(-1.,1.)),
        (Point::new(0.,0.),Point::new(0.,1.)),(Point::new(-1.,0.),Point::new(-0.5,1.))] {
        for (a,b) in [(a,b),(b,a)] {
            let edge=Edge {a,b,region:0};let mut work=Work {left:1000,cancelled:&||false};
            if edge_computed_x_stays_left(edge,0.,1.,0.,&mut work).unwrap() {
                for i in 0..=1024 {assert!(edge.x(i as f64/1024.)<=0.);}
            }
        }
    }
}

#[test]
fn left_edge_events_refuse_every_cancel_short_budget_and_storage_without_leaks() {
    use kurbo::Shape;
    use std::cell::Cell;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let unit=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);
    let mut p=BezPath::new();p.move_to((-2.,-0.1));p.line_to((-1.,0.2));p.line_to((0.8,0.4));p.line_to((-1.,0.7));p.line_to((-2.,1.1));p.close_path();
    let regions=[(&unit,Fill::NonZero),(&p,Fill::EvenOdd)];let live=Arc::new(AtomicUsize::new(0));
    let admit=|n| {live.fetch_add(n,Ordering::Relaxed);Some(Box::new(Credit(live.clone(),n)) as Box<dyn std::any::Any+Send+Sync>)};
    let row=admitted_selected_polygon_pixel_row_budget(&regions,0,0,&mut 1_000_000,&||false,&admit).unwrap();let held=live.load(Ordering::Relaxed);
    let run=|left,cancelled:&dyn Fn()->bool,storage:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>| {
        let mut work=Work {left,cancelled};
        let result=row.pixel_area_with_work_policy::<false>(&regions,0,0,&mut work,storage,Some(&mut |_|Some(())),
            Some(&mut |w|Some(w.iter().rposition(|&v|v!=0))),None,None,0,None,true);
        (result,work.left)
    };
    let calls=Cell::new(0);let (result,left)=run(1_000_000,&||{calls.set(calls.get()+1);false},&admit);assert!(result.is_some());assert_eq!(live.load(Ordering::Relaxed),held);
    for stop in 0..calls.get() {let n=Cell::new(0);assert!(run(1_000_000,&||{let old=n.get();n.set(old+1);old>=stop},&admit).0.is_none());assert_eq!(live.load(Ordering::Relaxed),held);}
    for limit in 0..1_000_000-left {let (result,remaining)=run(limit,&||false,&admit);assert!(result.is_none());assert!(remaining<=limit);assert_eq!(live.load(Ordering::Relaxed),held);}
    assert!(run(1_000_000,&||false,&|_|None).0.is_none());assert_eq!(live.load(Ordering::Relaxed),held);drop(row);assert_eq!(live.load(Ordering::Relaxed),0);
}
