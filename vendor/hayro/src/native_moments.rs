//! Admitted, bounded fragment moments. A matching key does not itself prove
//! affine color or convex ownership; those remain the caller's obligations.
use kurbo::Point;

#[derive(Clone,Copy,Default)]
struct Sum {value:f64,error:f64}
impl Sum {
    fn added(self,value:f64)->Option<Self> {
        let adjusted=value-self.error;
        let total=self.value+adjusted;
        let error=(total-self.value)-adjusted;
        (total.is_finite() && error.is_finite()).then_some(Self {value:total,error})
    }
}
/// Outward sum enclosure for exact products of reported sample values.
/// This excludes errors in clipping, source sampling and component conversion.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) struct IntegralEnvelope<const C:usize> {
    alpha:[f64;2],components:[[f64;2];C],max_component_abs:[f64;C],
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) struct IntegralRoundingBounds<const C:usize> {
    pub(super) alpha:[f64;2],pub(super) components:[[f64;2];C],
    pub(super) alpha_error:f64,pub(super) component_error:[f64;C],
    pub(super) max_component_abs:[f64;C],
}
impl<const C:usize> IntegralEnvelope<C> {
    pub(super) fn new()->Self {Self {alpha:[0.;2],components:[[0.;2];C],max_component_abs:[0.;C]}}
    pub(super) fn added(self,area:f64,alpha:f64,components:[f64;C],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        self.added_impl(area,alpha,components,1.,work,cancelled)
    }
    /// Moment aggregate area may exceed1by the already admitted rounding
    /// tolerance. Original individual-fragment validation remains strict.
    pub(super) fn added_aggregate(self,area:f64,alpha:f64,components:[f64;C],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        self.added_impl(area,alpha,components,1.+1e-12,work,cancelled)
    }
    fn added_impl(self,area:f64,alpha:f64,components:[f64;C],max_area:f64,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        debit(work,5+C.checked_mul(4)?,cancelled)?;
        if !(1..=4).contains(&C) || !area.is_finite() || area<=0. || area>max_area || !alpha.is_finite() || alpha<0. || alpha>1.+1e-12
            || components.iter().any(|v|!v.is_finite()) {return None;}
        let product=area*alpha;
        let weight=[product.next_down().max(0.),product.next_up()];
        let sum=|a:[f64;2],b:[f64;2]|[(a[0]+b[0]).next_down(),(a[1]+b[1]).next_up()];
        let mut out=self;out.alpha=sum(self.alpha,weight);out.alpha[0]=out.alpha[0].max(0.);
        for channel in 0..C {
            if cancelled() {return None;}
            let a=weight[0]*components[channel];let b=weight[1]*components[channel];
            let value=[a.min(b).next_down(),a.max(b).next_up()];
            out.components[channel]=sum(self.components[channel],value);
            out.max_component_abs[channel]=self.max_component_abs[channel].max(components[channel].abs());
        }
        if out.alpha.iter().chain(out.components.iter().flatten()).any(|v|!v.is_finite()) || cancelled() {return None;}
        Some(out)
    }
    /// Merge two independently enclosed sample streams. Their maximum native
    /// values remain a bound for the combined nonnegative-alpha weighted mean.
    pub(super) fn merged(self,other:Self,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        debit(work,1+C.checked_mul(4)?,cancelled)?;
        if !(1..=4).contains(&C) {return None;}
        let sum=|a:[f64;2],b:[f64;2]|[(a[0]+b[0]).next_down(),(a[1]+b[1]).next_up()];
        let mut out=self;out.alpha=sum(self.alpha,other.alpha);out.alpha[0]=out.alpha[0].max(0.);
        for channel in 0..C {
            if cancelled() {return None;}
            out.components[channel]=sum(self.components[channel],other.components[channel]);
            out.max_component_abs[channel]=self.max_component_abs[channel].max(other.max_component_abs[channel]);
        }
        if out.alpha.iter().chain(out.components.iter().flatten()).any(|v|!v.is_finite()) || cancelled() {return None;}
        Some(out)
    }
    /// Add a proved interval for an already accumulated integral. The supplied
    /// native-value bounds must independently bound that integral's exact
    /// nonnegative-alpha weighted mean; they are not inferred from the interval.
    pub(super) fn added_integral_bounds(self,alpha:[f64;2],components:[[f64;2];C],native_bounds:[f64;C],
        work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        debit(work,1+C.checked_mul(4)?,cancelled)?;
        if !(1..=4).contains(&C) || !alpha[0].is_finite() || !alpha[1].is_finite() || alpha[0]>alpha[1] || alpha[1]<0.
            || components.iter().any(|[lo,hi]|!lo.is_finite() || !hi.is_finite() || lo>hi)
            || native_bounds.iter().any(|v|!v.is_finite() || *v<0.) {return None;}
        let sum=|a:[f64;2],b:[f64;2]|[(a[0]+b[0]).next_down(),(a[1]+b[1]).next_up()];
        let mut out=self;out.alpha=sum(self.alpha,[alpha[0].max(0.),alpha[1]]);out.alpha[0]=out.alpha[0].max(0.);
        for channel in 0..C {
            if cancelled() {return None;}
            out.components[channel]=sum(self.components[channel],components[channel]);
            out.max_component_abs[channel]=self.max_component_abs[channel].max(native_bounds[channel]);
        }
        if out.alpha.iter().chain(out.components.iter().flatten()).any(|v|!v.is_finite()) || cancelled() {return None;}
        Some(out)
    }
    pub(super) fn finish(self,alpha:f64,components:[f64;C],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<IntegralRoundingBounds<C>> {
        debit(work,1+C.checked_mul(3)?,cancelled)?;
        if !alpha.is_finite() || components.iter().any(|v|!v.is_finite()) {return None;}
        let error=|v:f64,b:[f64;2]|(v-b[0]).abs().max((v-b[1]).abs()).next_up();
        let component_error=std::array::from_fn(|i|error(components[i],self.components[i]));
        let alpha_error=error(alpha,self.alpha);
        if !alpha_error.is_finite() || component_error.iter().any(|v|!v.is_finite()) || cancelled() {return None;}
        Some(IntegralRoundingBounds {alpha:self.alpha,components:self.components,alpha_error,component_error,max_component_abs:self.max_component_abs})
    }
}
/// Quotient discrepancy with differing alpha: (integral_error+B*alpha_error)
/// divided by a positive lower bound for both alphas. B bounds the exact
/// reference quotient. Covers division rounding on both compared quotients;
/// excludes later Pixel storage, profile conversion and display quantization.
pub(super) fn native_quotient_error_bound<const C:usize>(integral_error:[f64;C],alpha_error:f64,
    alpha_lower:f64,value_bounds:[f64;C],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f64;C]> {
    debit(work,1+C.checked_mul(8)?,cancelled)?;
    if !(1..=4).contains(&C) || !alpha_error.is_finite() || alpha_error<0. || !alpha_lower.is_finite() || alpha_lower<=0.
        || integral_error.iter().chain(value_bounds.iter()).any(|v|!v.is_finite() || *v<0.) {return None;}
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    let mut result=[0.;C];
    for channel in 0..C {
        if cancelled() {return None;}
        let base=(add(integral_error[channel],mul(value_bounds[channel],alpha_error))/alpha_lower).next_up();
        let magnitude=add(mul(2.,value_bounds[channel]),base);
        let rounding=add(mul(2.*f64::EPSILON,magnitude),2.*f64::from_bits(1));
        result[channel]=add(base,rounding);
        if !result[channel].is_finite() {return None;}
    }
    if cancelled() {return None;}Some(result)
}

#[derive(Clone,Copy,Default)]
struct GeometryBounds {area:[f64;2],x:[f64;2],y:[f64;2]}
/// Exact reported-fragment area/centroid sum enclosure, not polygon clipping error.
#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) struct GeometryRoundingBounds {
    pub(super) area:[f64;2],
    pub(super) centroid:[[f64;2];2],
    pub(super) area_error:f64,
    pub(super) centroid_error:[f64;2],
}
/// Complete numerical inputs for constant fields or an opaque native Normal
/// tail, with exact identity fallbacks retained. This is not a PDF/display proof.
#[derive(Clone,Copy)]
pub(super) struct OpaqueCandidateInputs {
    pub(super) original:IntegralRoundingBounds<4>,pub(super) candidate:IntegralRoundingBounds<4>,
    pub(super) arithmetic_native:[f64;4],pub(super) variable_area_original:f64,pub(super) variable_area_candidate:f64,
    pub(super) weighted_centroid_error:[f64;2],pub(super) total_area_error:f64,
    pub(super) original_variable_points:usize,pub(super) candidate_variable_points:usize,
}
/// Combine source/opaque-tail arithmetic, area/centroid and ratio errors.
/// Caller certifies source coverage and the opaque-tail route profile before
/// passing inputs. Result excludes later Pixel storage/profile/display changes.
pub(super) fn opaque_candidate_native_error(inputs:&OpaqueCandidateInputs,source_error:f64,gradient:[f64;2],
    work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f64;4]> {
    debit(work,64,cancelled)?;
    let values=[source_error,gradient[0],gradient[1],inputs.variable_area_original,inputs.variable_area_candidate,
        inputs.weighted_centroid_error[0],inputs.weighted_centroid_error[1],inputs.total_area_error];
    if values.iter().any(|v|!v.is_finite() || *v<0.) || source_error>0.25
        || inputs.arithmetic_native.iter().chain(inputs.original.max_component_abs.iter()).any(|v|!v.is_finite() || *v<0.) {return None;}
    for interval in [inputs.original.alpha,inputs.candidate.alpha] {
        if interval.iter().any(|v|!v.is_finite()) || interval[0]>interval[1] {return None;}
    }
    let lower=inputs.original.alpha[0].min(inputs.candidate.alpha[0]).next_down();
    if !lower.is_finite() || lower<=0. {return None;}
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    // An opaque final Normal paint overwrites its prefix. Same-coordinate
    // preparation, at most two Normal applications/batching and native storage
    // recovery are bounded by this conservative absolute f64 envelope.
    let point=add(source_error,1024.*f64::EPSILON);
    let source=mul(point,add(inputs.variable_area_original,inputs.variable_area_candidate));
    let geometry=add(add(mul(gradient[0],inputs.weighted_centroid_error[0]),mul(gradient[1],inputs.weighted_centroid_error[1])),mul(2.,inputs.total_area_error));
    let alpha=mul(2.,inputs.total_area_error);
    let numerator=add(source,geometry);let mut out=[0.;4];
    for channel in 0..4 {
        if cancelled() {return None;}
        let ratio=(add(numerator,mul(inputs.original.max_component_abs[channel],alpha))/lower).next_up();
        out[channel]=add(ratio,inputs.arithmetic_native[channel]);
        if !out[channel].is_finite() {return None;}
    }
    if cancelled() {return None;}Some(out)
}

/// Geometric replacement error for an exact affine field, comparing exact
/// reported-input moments with the table's evaluated area/centroid. This excludes
/// pointwise sampler error, geometry clipping error and integral summation error.
pub(super) fn affine_moment_geometry_error_bound<const C:usize>(geometry:&GeometryRoundingBounds,
    gradients:[[f64;2];C],value_bounds:[f64;C],remaining:&mut usize,cancelled:&dyn Fn()->bool,
)->Option<[f64;C]> {
    if cancelled() {return None;}*remaining=remaining.checked_sub(1+C.checked_mul(8)?)?;
    if !(1..=4).contains(&C) || geometry.area.iter().any(|v|!v.is_finite()) || geometry.area[0]<=0. || geometry.area[1]<geometry.area[0]
        || !geometry.area_error.is_finite() || geometry.area_error<0. || geometry.centroid_error.iter().any(|v|!v.is_finite() || *v<0.)
        || geometry.centroid.iter().any(|[lo,hi]|!lo.is_finite() || !hi.is_finite() || lo>hi)
        || gradients.iter().flatten().any(|v|!v.is_finite() || *v<0.) || value_bounds.iter().any(|v|!v.is_finite() || *v<0.) {return None;}
    let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();
    let mut out=[0.;C];
    for channel in 0..C {
        if cancelled() {return None;}
        let shift=add(mul(gradients[channel][0],geometry.centroid_error[0]),mul(gradients[channel][1],geometry.centroid_error[1]));
        out[channel]=add(mul(geometry.area[1],shift),mul(geometry.area_error,value_bounds[channel]));
        if !out[channel].is_finite() {return None;}
    }
    if cancelled() {return None;}Some(out)
}

impl GeometryBounds {
    fn added(self,area:f64,centroid:Point,origin:Point,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        debit(work,14,cancelled)?;
        let sum=|a:[f64;2],b:[f64;2]|[(a[0]+b[0]).next_down(),(a[1]+b[1]).next_up()];
        let moment=|point:f64,origin:f64| {
            let difference=[(point-origin).next_down(),(point-origin).next_up()];
            [(area*difference[0]).next_down(),(area*difference[1]).next_up()]
        };
        let out=Self {area:sum(self.area,[area;2]),x:sum(self.x,moment(centroid.x,origin.x)),y:sum(self.y,moment(centroid.y,origin.y))};
        [out.area,out.x,out.y].iter().flatten().all(|v|v.is_finite()).then_some(out)
    }
    fn finish(self,origin:Point,area:f64,centroid:Point)->Option<GeometryRoundingBounds> {
        if self.area[0]<=0. {return None;}
        let coordinate=|moment:[f64;2],origin:f64| {
            let ratios=[moment[0]/self.area[0],moment[0]/self.area[1],moment[1]/self.area[0],moment[1]/self.area[1]];
            let lo=ratios.iter().copied().fold(f64::INFINITY,f64::min).next_down();
            let hi=ratios.iter().copied().fold(f64::NEG_INFINITY,f64::max).next_up();
            [(origin+lo).next_down(),(origin+hi).next_up()]
        };
        let bounds=[coordinate(self.x,origin.x),coordinate(self.y,origin.y)];
        if bounds.iter().flatten().any(|v|!v.is_finite()) {return None;}
        let error=|value:f64,bounds:[f64;2]|(value-bounds[0]).abs().max((value-bounds[1]).abs()).next_up();
        let out=GeometryRoundingBounds {area:self.area,centroid:bounds,area_error:error(area,self.area),centroid_error:[error(centroid.x,bounds[0]),error(centroid.y,bounds[1])]};
        (out.area_error.is_finite() && out.centroid_error.iter().all(|e|e.is_finite())).then_some(out)
    }
}

// Reference values are reported f64values. One outward input step also
// encloses a single upstream multiplication (native reference alpha*component).
fn accumulated_reference_bounds<const K:usize>(previous:[[f64;2];K],area:f64,values:[f64;K],
    work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f64;2];K]> {
    debit(work,1+K.checked_mul(4)?,cancelled)?;let mut out=previous;
    for channel in 0..K {
        if cancelled() {return None;}
        let lo=(area*values[channel].next_down()).next_down();
        let hi=(area*values[channel].next_up()).next_up();
        out[channel]=[(previous[channel][0]+lo).next_down(),(previous[channel][1]+hi).next_up()];
        if out[channel].iter().any(|v|!v.is_finite()) {return None;}
    }
    if cancelled() {return None;}Some(out)
}

struct Entry<const N:usize,const K:usize,const W:usize> {key:[u64;N],words:usize,witness:[u64;W],area:Sum,x:Sum,y:Sum,reference:[Sum;K]}
pub(super) struct MomentTable<const N:usize,const K:usize=0,const W:usize=0> {
    entries:Vec<Entry<N,K,W>>,
    slots:Vec<usize>,
    capacity:usize,
    words:usize,
    origin:Point,
    total:Sum,
    geometry_bounds:Option<Vec<GeometryBounds>>,
    reference_bounds:Option<Vec<[[f64;2];K]>>,
    _reference_credit:Option<Box<dyn std::any::Any+Send+Sync>>,
    _geometry_credit:Option<Box<dyn std::any::Any+Send+Sync>>,
    _credit:Box<dyn std::any::Any+Send+Sync>,
}
fn debit(remaining:&mut usize,count:usize,cancelled:&dyn Fn()->bool)->Option<()> {
    if cancelled() {return None;}
    *remaining=remaining.checked_sub(count)?;Some(())
}
impl<const N:usize,const K:usize,const W:usize> MomentTable<N,K,W> {
    pub(super) fn admitted(capacity:usize,origin:Point,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
    )->Option<Self> {
        Self::admitted_prefix(capacity,origin,N,admit)
    }
    pub(super) fn admitted_prefix(capacity:usize,origin:Point,words:usize,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
    )->Option<Self> {
        if words==0 || words>N {return None;}
        if !(1..=64).contains(&N) || K>5 || W>64 || !(1..=16384).contains(&capacity)
            || !origin.x.is_finite() || !origin.y.is_finite()
            || origin.x.abs()>1_048_576. || origin.y.abs()>1_048_576. {return None;}
        let slots=capacity.checked_mul(2)?.checked_next_power_of_two()?;
        let bytes=capacity.checked_mul(size_of::<Entry<N,K,W>>())?
            .checked_add(slots.checked_mul(size_of::<usize>())?)?;
        let credit=admit(bytes)?;
        let mut entries=Vec::new();entries.try_reserve_exact(capacity).ok()?;
        let mut index=Vec::new();index.try_reserve_exact(slots).ok()?;index.resize(slots,usize::MAX);
        if entries.capacity()>capacity || index.capacity()>slots {return None;}
        Some(Self {entries,slots:index,capacity,words,origin,total:Sum::default(),geometry_bounds:None,reference_bounds:None,_reference_credit:None,_geometry_credit:None,_credit:credit})
    }
    /// Opt-in separately admitted geometry interval storage. Ordinary tables
    /// retain their original entry size and perform no interval arithmetic.
    pub(super) fn admitted_prefix_with_geometry_bounds(capacity:usize,origin:Point,words:usize,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
    )->Option<Self> {
        let mut table=Self::admitted_prefix(capacity,origin,words,admit)?;
        let credit=admit(capacity.checked_mul(size_of::<GeometryBounds>())?)?;
        let mut bounds=Vec::new();bounds.try_reserve_exact(capacity).ok()?;
        if bounds.capacity()>capacity {return None;}
        table.geometry_bounds=Some(bounds);table._geometry_credit=Some(credit);Some(table)
    }
    /// Separately admitted reference intervals; exact reported values or one
    /// upstream correctly rounded product are enclosed before area weighting.
    pub(super) fn admitted_prefix_with_reference_bounds(capacity:usize,origin:Point,words:usize,geometry:bool,
        admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>,
    )->Option<Self> {
        if K==0 {return None;}
        let mut table=if geometry {Self::admitted_prefix_with_geometry_bounds(capacity,origin,words,admit)?}
            else {Self::admitted_prefix(capacity,origin,words,admit)?};
        let credit=admit(capacity.checked_mul(size_of::<[[f64;2];K]>())?)?;
        let mut intervals=Vec::new();intervals.try_reserve_exact(capacity).ok()?;
        if intervals.capacity()>capacity {return None;}
        table.reference_bounds=Some(intervals);table._reference_credit=Some(credit);Some(table)
    }
    pub(super) fn reference_integral_bounds(&self,index:usize,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[[f64;2];K]> {
        debit(work,1+K,cancelled)?;
        let bounds=*self.reference_bounds.as_ref()?.get(index)?;
        if cancelled() {return None;}Some(bounds)
    }
    pub(super) fn geometry_rounding_bounds(&self,index:usize,remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<GeometryRoundingBounds> {
        debit(remaining,32,cancelled)?;
        let entry=self.entries.get(index)?;
        let centroid=Point::new(self.origin.x+entry.x.value/entry.area.value,self.origin.y+entry.y.value/entry.area.value);
        let bounds=self.geometry_bounds.as_ref()?.get(index)?.finish(self.origin,entry.area.value,centroid)?;
        if cancelled() {return None;}Some(bounds)
    }
    pub(super) fn add(&mut self,key:[u64;N],area:f64,centroid:Point,
        remaining:&mut usize,cancelled:&dyn Fn()->bool,
    )->Option<()> {
        self.add_sample(key,area,centroid,[0.;K],remaining,cancelled)
    }
    pub(super) fn add_sample(&mut self,key:[u64;N],area:f64,centroid:Point,reference:[f64;K],
        remaining:&mut usize,cancelled:&dyn Fn()->bool,
    )->Option<()> {
        self.add_prefix_sample(&key,area,centroid,reference,remaining,cancelled)
    }
    /// Prefix width is fixed at admission. Omitted words are stored as zero;
    /// wrong-width keys refuse rather than silently truncating identity.
    pub(super) fn add_prefix_sample(&mut self,key:&[u64],area:f64,centroid:Point,reference:[f64;K],
        remaining:&mut usize,cancelled:&dyn Fn()->bool,
    )->Option<()> {
        self.add_impl(key,false,None,&[0;W],area,centroid,reference,remaining,cancelled)
    }
    /// Caller supplies a stable hash of the complete key and accounts for its
    /// computation. Collisions always require exact key equality. Never mix
    /// hash schemes for one table: equal keys must receive equal hashes.
    pub(super) fn add_prehashed_sample(&mut self,key:&[u64],hash:u64,area:f64,centroid:Point,reference:[f64;K],
        remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        self.add_impl(key,false,Some(hash),&[0;W],area,centroid,reference,remaining,cancelled)
    }
    /// Retain the first representative only. Caller must prove that its
    /// interpretation remains valid for every fragment with this identity.
    pub(super) fn add_witness_sample(&mut self,key:&[u64],prehash:Option<u64>,witness:&[u64;W],area:f64,
        centroid:Point,reference:[f64;K],remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        self.add_impl(key,false,prehash,witness,area,centroid,reference,remaining,cancelled)
    }
    /// Variable identity uses every supplied word and its exact length. The
    /// admission width is a maximum, not permission to truncate a longer key.
    pub(super) fn add_variable_witness_sample(&mut self,key:&[u64],prehash:Option<u64>,witness:&[u64;W],area:f64,
        centroid:Point,reference:[f64;K],remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        self.add_impl(key,true,prehash,witness,area,centroid,reference,remaining,cancelled)
    }
    fn add_impl(&mut self,key:&[u64],variable:bool,prehash:Option<u64>,witness:&[u64;W],area:f64,centroid:Point,reference:[f64;K],
        remaining:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        if key.is_empty() || key.len()>self.words || (!variable && key.len()!=self.words) {return None;}
        debit(remaining,8+K,cancelled)?;
        if reference.iter().any(|v|!v.is_finite()) {return None;}
        let x=centroid.x-self.origin.x;let y=centroid.y-self.origin.y;
        if !area.is_finite() || area<=0. || area>1. || !x.is_finite() || !y.is_finite()
            || !(0. ..=1.).contains(&x) || !(0. ..=1.).contains(&y) {return None;}
        let total=self.total.added(area)?;
        if total.value>1.+1e-12 {return None;}
        let mut hash=if let Some(hash)=prehash {hash} else {
            debit(remaining,key.len(),cancelled)?;
            let mut hash=0xcbf29ce484222325u64;
            for &word in key {hash=(hash^word).wrapping_mul(0x100000001b3);}
            hash
        };
        debit(remaining,3,cancelled)?;
        hash=(hash^(hash>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
        hash=(hash^(hash>>27)).wrapping_mul(0x94d049bb133111eb);hash^=hash>>31;
        let mut slot=hash as usize&(self.slots.len()-1);
        for _ in 0..self.slots.len() {
            debit(remaining,1,cancelled)?;
            let index=self.slots[slot];
            if index==usize::MAX {
                if self.entries.len()==self.capacity {return None;}
                debit(remaining,N.checked_add(key.len())?.checked_add(W)?.checked_add(4+K)?,cancelled)?;
                let mut values=[Sum::default();K];
                for (sum,value) in values.iter_mut().zip(reference) {*sum=sum.added(area*value)?;}
                let mut stored=[0u64;N];stored[..key.len()].copy_from_slice(key);
                let entry=Entry {key:stored,words:key.len(),witness:*witness,area:Sum::default().added(area)?,
                    x:Sum::default().added(area*x)?,y:Sum::default().added(area*y)?,reference:values};
                let geometry=if self.geometry_bounds.is_some() {Some(GeometryBounds::default().added(area,centroid,self.origin,remaining,cancelled)?)} else {None};
                let reference_interval=if self.reference_bounds.is_some() {Some(accumulated_reference_bounds([[0.;2];K],area,reference,remaining,cancelled)?)} else {None};
                if cancelled() {return None;}
                self.slots[slot]=self.entries.len();self.entries.push(entry);self.total=total;
                if let (Some(bounds),Some(geometry))=(&mut self.geometry_bounds,geometry) {bounds.push(geometry);}
                if let (Some(bounds),Some(reference_interval))=(&mut self.reference_bounds,reference_interval) {bounds.push(reference_interval);}
                return Some(());
            }
            debit(remaining,1,cancelled)?;
            if self.entries[index].words!=key.len() {slot=(slot+1)&(self.slots.len()-1);continue;}
            debit(remaining,key.len(),cancelled)?;
            if self.entries[index].key[..key.len()]==*key {
                debit(remaining,3+K,cancelled)?;
                let entry=&self.entries[index];
                let a=entry.area.added(area)?;let sx=entry.x.added(area*x)?;let sy=entry.y.added(area*y)?;
                let mut values=entry.reference;
                for (sum,value) in values.iter_mut().zip(reference) {*sum=sum.added(area*value)?;}
                let geometry=if let Some(bounds)=&self.geometry_bounds {Some(bounds[index].added(area,centroid,self.origin,remaining,cancelled)?)} else {None};
                let reference_interval=if let Some(bounds)=&self.reference_bounds {Some(accumulated_reference_bounds(bounds[index],area,reference,remaining,cancelled)?)} else {None};
                if cancelled() {return None;}
                let entry=&mut self.entries[index];entry.area=a;entry.x=sx;entry.y=sy;entry.reference=values;self.total=total;
                if let (Some(bounds),Some(geometry))=(&mut self.geometry_bounds,geometry) {bounds[index]=geometry;}
                if let (Some(bounds),Some(reference_interval))=(&mut self.reference_bounds,reference_interval) {bounds[index]=reference_interval;}
                return Some(());
            }
            slot=(slot+1)&(self.slots.len()-1);
        }
        None
    }
    pub(super) fn iter_with_witness(&self)->impl Iterator<Item=([u64;N],f64,Point,[f64;K],[u64;W],usize)>+'_ {
        self.entries.iter().map(|e|(e.key,e.area.value,
            Point::new(self.origin.x+e.x.value/e.area.value,self.origin.y+e.y.value/e.area.value),e.reference.map(|v|v.value),e.witness,e.words))
    }
    /// Fixed-width view. Variable-key callers must use iter_with_witness to
    /// retain the explicit identity length as well as its zero-padded words.
    pub(super) fn iter_with_reference(&self)->impl Iterator<Item=([u64;N],f64,Point,[f64;K])>+'_ {
        self.entries.iter().map(|e|(e.key,e.area.value,
            Point::new(self.origin.x+e.x.value/e.area.value,self.origin.y+e.y.value/e.area.value),e.reference.map(|v|v.value)))
    }
    /// Fixed-width view; use iter_with_witness for variable-length identity.
    pub(super) fn iter(&self)->impl Iterator<Item=([u64;N],f64,Point)>+'_ {
        self.entries.iter().map(|e|(e.key,e.area.value,
            Point::new(self.origin.x+e.x.value/e.area.value,self.origin.y+e.y.value/e.area.value)))
    }
}

#[test]
fn moments_integrate_affine_fields_and_keep_keys_independent() {
    let mut table=MomentTable::<2>::admitted(8,Point::new(1000.,-2000.),&|_|Some(Box::new(()))).unwrap();
    let mut work=1_000_000;
    for i in 0..4096 {
        let x=(i as f64+0.5)/4096.;let area=1./4096.;
        table.add([i%2,17],area,Point::new(1000.+x,-1999.5),&mut work,&||false).unwrap();
    }
    let mut integral=0.;let mut area=0.;
    for (_,a,p) in table.iter() {area+=a;integral+=a*(0.1+0.4*(p.x-1000.)+0.2*(p.y+2000.));}
    assert_eq!(area,1.);assert!((integral-0.4).abs()<1e-14);
    assert_eq!(table.iter().count(),2);
    let mut triangle=MomentTable::<1>::admitted(1,Point::ZERO,&|_|Some(Box::new(()))).unwrap();
    for (x,y) in [(4./9.,1./9.),(4./9.,4./9.),(1./9.,4./9.)] {
        triangle.add([0],1./6.,Point::new(x,y),&mut work,&||false).unwrap();
    }
    let (_,a,p)=triangle.iter().next().unwrap();
    assert_eq!(a,0.5);assert!((p.x-1./3.).abs()<1e-15 && (p.y-1./3.).abs()<1e-15);
    assert!((a*(0.1+0.4*p.x+0.2*p.y)-0.15).abs()<1e-15);
}

#[test]
fn moment_refusal_and_cancellation_preserve_entries_and_release_admission() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    let admit=|bytes|{used.fetch_add(bytes,Ordering::Relaxed);
        Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    assert!(MomentTable::<1>::admitted(1,Point::ZERO,&|_|None).is_none());
    {
        let mut table=MomentTable::<1>::admitted(1,Point::ZERO,&admit).unwrap();
        let mut work=10000;table.add([1],0.25,Point::new(0.25,0.5),&mut work,&||false).unwrap();
        let expected=table.iter().collect::<Vec<_>>();
        assert!(table.add([2],0.25,Point::new(0.75,0.5),&mut work,&||false).is_none());
        assert_eq!(table.iter().collect::<Vec<_>>(),expected);
        for budget in 0..16 {
            assert!(table.add([1],0.25,Point::new(0.75,0.5),&mut {budget},&||false).is_none());
            assert_eq!(table.iter().collect::<Vec<_>>(),expected);
        }
        let polls=std::cell::Cell::new(0);
        assert!(table.add([1],0.25,Point::new(0.75,0.5),&mut work,
            &||{polls.set(polls.get()+1);polls.get()==3}).is_none());
        assert_eq!(table.iter().collect::<Vec<_>>(),expected);
        for (area,p) in [(f64::NAN,Point::ZERO),(0.,Point::ZERO),(1.1,Point::ZERO),
            (0.1,Point::new(-0.1,0.5)),(0.1,Point::new(f64::INFINITY,0.))] {
            assert!(table.add([1],area,p,&mut work,&||false).is_none());
            assert_eq!(table.iter().collect::<Vec<_>>(),expected);
        }
        table.add([1],0.25,Point::new(0.75,0.5),&mut work,&||false).unwrap();
        assert_eq!(table.iter().next().unwrap(),([1],0.5,Point::new(0.5,0.5)));
    }
    assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn optional_reference_integrals_preserve_transactional_updates() {
    let mut table=MomentTable::<1,5>::admitted(2,Point::ZERO,&|_|Some(Box::new(()))).unwrap();
    let mut work=10000;
    table.add_sample([1],0.25,Point::new(0.25,0.5),[1.,0.25,0.5,0.75,1.],&mut work,&||false).unwrap();
    let expected=table.iter_with_reference().collect::<Vec<_>>();
    assert!(table.add_sample([1],0.25,Point::new(0.75,0.5),[1.,f64::NAN,0.,0.,0.],&mut work,&||false).is_none());
    assert_eq!(table.iter_with_reference().collect::<Vec<_>>(),expected);
    assert!(table.add_sample([1],0.25,Point::new(0.75,0.5),[1.;5],&mut 1,&||false).is_none());
    assert_eq!(table.iter_with_reference().collect::<Vec<_>>(),expected);
    table.add_sample([1],0.25,Point::new(0.75,0.5),[1.,0.75,0.5,0.25,0.],&mut work,&||false).unwrap();
    assert_eq!(table.iter_with_reference().next().unwrap(),([1],0.5,Point::new(0.5,0.5),[0.5,0.25,0.25,0.25,0.25]));
}

#[test]
fn prefix_identity_rejects_truncation_and_preserves_each_active_word() {
    let mut table=MomentTable::<64,1>::admitted_prefix(4,Point::ZERO,2,&|_|Some(Box::new(()))).unwrap();
    let mut work=10000;
    table.add_prefix_sample(&[1,2],0.25,Point::new(0.25,0.5),[0.5],&mut work,&||false).unwrap();
    table.add_prefix_sample(&[1,3],0.25,Point::new(0.75,0.5),[1.],&mut work,&||false).unwrap();
    let before=table.iter_with_reference().collect::<Vec<_>>();
    assert_eq!(before.len(),2);
    for (key,_,_,_) in &before {assert!(key[2..].iter().all(|&v|v==0));}
    for key in [&[1][..],&[1,2,99][..],&[0;64][..]] {
        assert!(table.add_prefix_sample(key,0.1,Point::new(0.5,0.5),[1.],&mut work,&||false).is_none());
        assert_eq!(table.iter_with_reference().collect::<Vec<_>>(),before);
    }
    assert!(table.add_sample([0;64],0.1,Point::new(0.5,0.5),[1.],&mut work,&||false).is_none());
    table.add_prefix_sample(&[1,2],0.25,Point::new(0.75,0.5),[0.5],&mut work,&||false).unwrap();
    let entries=table.iter_with_reference().collect::<Vec<_>>();
    assert_eq!(entries.len(),2);assert_eq!(entries[0].1,0.5);assert_eq!(entries[0].3,[0.25]);
    assert_eq!(entries[0].2,Point::new(0.5,0.5));
    assert!(MomentTable::<64>::admitted_prefix(1,Point::ZERO,65,&|_|Some(Box::new(()))).is_none());
}

/// XOR of deterministic per-bit tokens: toggling one coverage bit updates a
/// complete-key fingerprint. It is only an index, never an equality proof.
pub(super) fn coverage_bit_token(index:usize)->u64 {
    let mut hash=(index as u64).wrapping_add(0x9e3779b97f4a7c15);
    hash=(hash^(hash>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash=(hash^(hash>>27)).wrapping_mul(0x94d049bb133111eb);
    hash^(hash>>31)
}

#[test]
fn prehashed_collisions_keep_full_identity_and_incremental_toggle_parity() {
    let mut table=MomentTable::<2>::admitted(4,Point::ZERO,&|_|Some(Box::new(()))).unwrap();
    let mut work=10000;
    for key in [[1,0],[0,1],[1,0],[0,1]] {
        table.add_prehashed_sample(&key,17,0.25,Point::new(0.5,0.5),[],&mut work,&||false).unwrap();
    }
    let entries=table.iter().collect::<Vec<_>>();
    assert_eq!(entries.len(),2);assert_eq!(entries[0].1,0.5);assert_eq!(entries[1].1,0.5);
    let before=entries;
    assert!(table.add_prehashed_sample(&[1,0],17,0.1,Point::new(0.5,0.5),[],&mut 0,&||false).is_none());
    assert_eq!(table.iter().collect::<Vec<_>>(),before);
    let mut bits=[0u64;64];let mut hash=0u64;
    for i in 0..8192 {
        let index=(i*1543+11)%4096;
        bits[index/64]^=1u64<<(index%64);hash^=coverage_bit_token(index);
        let expected=bits.iter().enumerate().fold(0u64,|mut hash,(word,&bits)| {
            for bit in 0..64 {if bits&(1u64<<bit)!=0 {hash^=coverage_bit_token(word*64+bit);}}hash
        });
        assert_eq!(hash,expected);
    }
    assert_eq!(hash,0);assert_eq!(bits,[0;64]);
}

#[test]
fn moment_witness_retains_first_representative_transactionally() {
    let mut table=MomentTable::<1,1,2>::admitted(2,Point::ZERO,&|_|Some(Box::new(()))).unwrap();
    let mut work=1000;
    table.add_witness_sample(&[1],Some(9),&[11,12],0.25,Point::new(0.25,0.5),[0.5],&mut work,&||false).unwrap();
    let before=table.iter_with_witness().collect::<Vec<_>>();
    assert!(table.add_witness_sample(&[1],Some(9),&[99,98],0.25,Point::new(0.75,0.5),[0.5],&mut 0,&||false).is_none());
    assert_eq!(table.iter_with_witness().collect::<Vec<_>>(),before);
    table.add_witness_sample(&[1],Some(9),&[99,98],0.25,Point::new(0.75,0.5),[0.5],&mut work,&||false).unwrap();
    let entry=table.iter_with_witness().next().unwrap();assert_eq!(entry.4,[11,12]);assert_eq!(entry.1,0.5);
    assert!(MomentTable::<1,0,65>::admitted(1,Point::ZERO,&|_|Some(Box::new(()))).is_none());
}

#[test]
fn variable_keys_preserve_length_under_collision_and_fixed_width_refusal() {
    let mut table=MomentTable::<4,0,1>::admitted(4,Point::ZERO,&|_|Some(Box::new(()))).unwrap();
    let mut work=10000;
    for key in [&[7][..],&[7,0][..],&[7,0,0][..]] {
        table.add_variable_witness_sample(key,Some(9),&[key.len() as u64],0.125,Point::new(0.5,0.5),[],&mut work,&||false).unwrap();
    }
    let before=table.iter_with_witness().collect::<Vec<_>>();assert_eq!(before.len(),3);
    assert_eq!(before.iter().map(|e|e.5).collect::<Vec<_>>(),[1,2,3]);
    for key in [&[][..],&[7,0,0,0,0][..]] {
        assert!(table.add_variable_witness_sample(key,Some(9),&[99],0.1,Point::new(0.5,0.5),[],&mut work,&||false).is_none());
    }
    assert!(table.add_witness_sample(&[7],Some(9),&[99],0.1,Point::new(0.5,0.5),[],&mut work,&||false).is_none());
    assert!(table.add_variable_witness_sample(&[7],Some(9),&[99],0.1,Point::new(0.5,0.5),[],&mut 0,&||false).is_none());
    assert_eq!(table.iter_with_witness().collect::<Vec<_>>(),before);
    table.add_variable_witness_sample(&[7],Some(9),&[99],0.125,Point::new(0.5,0.5),[],&mut work,&||false).unwrap();
    let after=table.iter_with_witness().collect::<Vec<_>>();assert_eq!(after[0].1,0.25);assert_eq!(after[0].4,[1]);
    for e in after {assert!(e.0[e.5..].iter().all(|&v|v==0));}
}

#[test]
fn geometry_interval_moments_enclose_reported_weighted_centroids_and_do_not_change_values() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let origin=Point::new(1000.,-2000.);
    let mut bounded=MomentTable::<2>::admitted_prefix_with_geometry_bounds(2,origin,2,&admit).unwrap();
    let mut plain=MomentTable::<2>::admitted_prefix(2,origin,2,&admit).unwrap();
    for i in 0..4096 {
        let point=Point::new(1000.+(i as f64+0.5)/4096.,-1999.5);
        bounded.add([1,7],1./4096.,point,&mut 1_000_000,&||false).unwrap();
        plain.add([1,7],1./4096.,point,&mut 1_000_000,&||false).unwrap();
    }
    assert_eq!(bounded.iter().collect::<Vec<_>>(),plain.iter().collect::<Vec<_>>());
    let bound=bounded.geometry_rounding_bounds(0,&mut 32,&||false).unwrap();
    assert!(bound.area[0]<=1. && bound.area[1]>=1.);
    for (value,[lo,hi]) in [1000.5,-1999.5].into_iter().zip(bound.centroid) {assert!(lo<=value && value<=hi);}
    assert!(bound.area_error<1e-10);assert!(bound.centroid_error.iter().all(|e|*e<1e-8));
    assert!(plain.geometry_rounding_bounds(0,&mut 32,&||false).is_none());
}
#[test]
fn geometry_interval_mutation_refusal_and_cancellation_preserve_both_moment_states() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let make=|| {let mut t=MomentTable::<1>::admitted_prefix_with_geometry_bounds(1,Point::ZERO,1,&admit).unwrap();t.add([1],0.25,Point::new(0.25,0.5),&mut 1000,&||false).unwrap();t};
    let mut probe=make();let mut work=1000;probe.add([1],0.25,Point::new(0.75,0.5),&mut work,&||false).unwrap();let cost=1000-work;
    for budget in 0..cost {
        let mut table=make();let before=table.geometry_rounding_bounds(0,&mut 32,&||false).unwrap();let values=table.iter().collect::<Vec<_>>();
        assert!(table.add([1],0.25,Point::new(0.75,0.5),&mut {budget},&||false).is_none());
        assert_eq!(table.geometry_rounding_bounds(0,&mut 32,&||false),Some(before));assert_eq!(table.iter().collect::<Vec<_>>(),values);
    }
    let polls=std::cell::Cell::new(0);let mut probe=make();probe.add([1],0.25,Point::new(0.75,0.5),&mut 1000,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {
        let mut table=make();let before=table.geometry_rounding_bounds(0,&mut 32,&||false).unwrap();
        let count=std::cell::Cell::new(0);assert!(table.add([1],0.25,Point::new(0.75,0.5),&mut 1000,&||{count.set(count.get()+1);count.get()>=stop}).is_none());
        assert_eq!(table.geometry_rounding_bounds(0,&mut 32,&||false),Some(before));
    }
}

#[test]
fn geometry_interval_side_storage_admission_releases_on_refusal_and_drop() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));let calls=std::cell::Cell::new(0);
    let deny_side=|bytes| {calls.set(calls.get()+1);if calls.get()==2 {return None;}used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    assert!(MomentTable::<1>::admitted_prefix_with_geometry_bounds(2,Point::ZERO,1,&deny_side).is_none());
    assert_eq!(used.load(Ordering::Relaxed),0);
    let admit=|bytes| {used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    let table=MomentTable::<1>::admitted_prefix_with_geometry_bounds(2,Point::ZERO,1,&admit).unwrap();
    let side=table.geometry_bounds.as_ref().unwrap();assert_eq!(side.capacity(),2);assert!(side.is_empty());
    assert!(used.load(Ordering::Relaxed)>=2*size_of::<GeometryBounds>());
    drop(table);assert_eq!(used.load(Ordering::Relaxed),0);
}

#[test]
fn geometry_interval_first_insertion_refusal_is_transactional_and_retry_succeeds() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let make=||MomentTable::<1>::admitted_prefix_with_geometry_bounds(1,Point::ZERO,1,&admit).unwrap();
    let mut probe=make();let mut work=1000;
    probe.add([1],0.25,Point::new(0.25,0.5),&mut work,&||false).unwrap();
    let cost=1000-work;
    for budget in 0..cost {
        let mut table=make();
        assert!(table.add([1],0.25,Point::new(0.25,0.5),&mut {budget},&||false).is_none());
        assert!(table.entries.is_empty());assert!(table.geometry_bounds.as_ref().unwrap().is_empty());
        assert!(table.slots.iter().all(|i|*i==usize::MAX));
        table.add([1],0.25,Point::new(0.25,0.5),&mut 1000,&||false).unwrap();
        assert!(table.geometry_rounding_bounds(0,&mut 32,&||false).is_some());
    }
    let polls=std::cell::Cell::new(0);let mut probe=make();probe.add([1],0.25,Point::new(0.25,0.5),&mut 1000,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {
        let count=std::cell::Cell::new(0);let mut table=make();
        assert!(table.add([1],0.25,Point::new(0.25,0.5),&mut 1000,&||{count.set(count.get()+1);count.get()>=stop}).is_none());
        assert!(table.entries.is_empty());assert!(table.geometry_bounds.as_ref().unwrap().is_empty());
        assert!(table.slots.iter().all(|i|*i==usize::MAX));
    }
}

#[test]
fn affine_moment_geometry_error_bounds_enclose_area_and_centroid_corner_perturbations() {
    let geometry=GeometryRoundingBounds {area:[0.99,1.01],centroid:[[0.48,0.52],[0.47,0.53]],area_error:0.011,centroid_error:[0.021,0.031]};
    let gradients=[[2.,3.],[0.5,1.]];let values=[3.,1.];let mut work=100;
    let bounds=affine_moment_geometry_error_bound(&geometry,gradients,values,&mut work,&||false).unwrap();
    for area in geometry.area {for x in geometry.centroid[0] {for y in geometry.centroid[1] {
        let exact=[area*(0.1+2.*x+3.*y),area*(0.25-0.5*x+y)];
        let evaluated=[0.1+2.*0.5+3.*0.5,0.25-0.5*0.5+0.5];
        for i in 0..2 {assert!((exact[i]-evaluated[i]).abs()<=bounds[i]);}
    }}}
    for budget in 0..100-work {assert!(affine_moment_geometry_error_bound(&geometry,gradients,values,&mut {budget},&||false).is_none());}
    let polls=std::cell::Cell::new(0);affine_moment_geometry_error_bound(&geometry,gradients,values,&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(affine_moment_geometry_error_bound(&geometry,gradients,values,&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
    assert!(affine_moment_geometry_error_bound(&geometry,[[f64::NAN,0.];2],values,&mut 100,&||false).is_none());
}

#[test]
fn integral_envelope_encloses_exact_dyadic_products_without_changing_naive_outputs() {
    let mut envelope=IntegralEnvelope::<2>::new();let mut alpha=0.;let mut integral=[0.;2];
    for i in 0..4096 {
        let components=[(i as f64+0.5)/4096.,0.25];let area=1./4096.;let opacity=0.5;
        let weight=area*opacity;alpha+=weight;for channel in 0..2 {integral[channel]+=weight*components[channel];}
        envelope=envelope.added(area,opacity,components,&mut 100,&||false).unwrap();
    }
    let bounds=envelope.finish(alpha,integral,&mut 100,&||false).unwrap();
    assert!(bounds.alpha[0]<=0.5 && bounds.alpha[1]>=0.5);
    for (exact,[lo,hi]) in [0.25,0.125].into_iter().zip(bounds.components) {assert!(lo<=exact && exact<=hi);}
    let native=native_quotient_error_bound(bounds.component_error,bounds.alpha_error,bounds.alpha[0].min(alpha).next_down(),bounds.max_component_abs,&mut 100,&||false).unwrap();
    for (i,exact) in [0.5,0.25].into_iter().enumerate() {assert!((integral[i]/alpha-exact).abs()<=native[i]);assert!(native[i]<1e-10);}
}
#[test]
fn integral_envelope_and_quotient_refuse_exhaustion_cancel_invalid_values() {
    let envelope=IntegralEnvelope::<2>::new();let values=[0.2,-0.1];
    let mut work=100;envelope.added(0.5,0.5,values,&mut work,&||false).unwrap();
    for budget in 0..100-work {assert!(envelope.added(0.5,0.5,values,&mut {budget},&||false).is_none());}
    let polls=std::cell::Cell::new(0);envelope.added(0.5,0.5,values,&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(envelope.added(0.5,0.5,values,&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
    let mut finish_work=100;envelope.finish(0.,[0.;2],&mut finish_work,&||false).unwrap();
    for budget in 0..100-finish_work {assert!(envelope.finish(0.,[0.;2],&mut {budget},&||false).is_none());}
    let finish_polls=std::cell::Cell::new(0);envelope.finish(0.,[0.;2],&mut 100,&||{finish_polls.set(finish_polls.get()+1);false}).unwrap();
    for stop in 1..=finish_polls.get() {let count=std::cell::Cell::new(0);assert!(envelope.finish(0.,[0.;2],&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
    assert!(envelope.added(0.5,-0.5,values,&mut 100,&||false).is_none());
    assert!(envelope.added(0.5,0.5,[f64::NAN,0.],&mut 100,&||false).is_none());
    assert!(native_quotient_error_bound([1e-9;2],1e-10,0.,[1.;2],&mut 100,&||false).is_none());
    assert!(native_quotient_error_bound([-1.;2],0.,0.5,[1.;2],&mut 100,&||false).is_none());
    assert!(native_quotient_error_bound([1.;2],0.,f64::from_bits(1),[1.;2],&mut 100,&||false).is_none());
    let mut work=100;native_quotient_error_bound([1e-9;2],1e-10,0.5,[1.;2],&mut work,&||false).unwrap();
    for budget in 0..100-work {assert!(native_quotient_error_bound([1e-9;2],1e-10,0.5,[1.;2],&mut {budget},&||false).is_none());}
    let polls=std::cell::Cell::new(0);native_quotient_error_bound([1e-9;2],1e-10,0.5,[1.;2],&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(native_quotient_error_bound([1e-9;2],1e-10,0.5,[1.;2],&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
}
#[test]
fn quotient_bound_contains_differing_alpha_comparisons_including_tiny_positive_alpha() {
    for alpha in [1e-300f64,1e-12,0.01,0.5] {for value in [0.,0.125,0.333333333333,0.9] {
        let other_alpha=alpha*1.1;let other_value=value+0.0001;
        let a=alpha*value;let b=other_alpha*other_value;
        let error=native_quotient_error_bound([(a-b).abs().next_up()],(alpha-other_alpha).abs().next_up(),alpha.min(other_alpha).next_down(),[2.],&mut 100,&||false).unwrap();
        assert!((a/alpha-b/other_alpha).abs()<=error[0]);
    }}
}

#[test]
fn merged_integral_envelopes_cover_two_sample_streams_and_all_refusal_checkpoints() {
    let left=IntegralEnvelope::<1>::new().added(0.5,0.5,[0.125],&mut 100,&||false).unwrap();
    let right=IntegralEnvelope::<1>::new().added(0.5,0.5,[0.875],&mut 100,&||false).unwrap();
    let mut work=100;let merged=left.merged(right,&mut work,&||false).unwrap();
    let bounds=merged.finish(0.5,[0.25],&mut 100,&||false).unwrap();
    assert!(bounds.alpha[0]<=0.5 && bounds.alpha[1]>=0.5);
    assert!(bounds.components[0][0]<=0.25 && bounds.components[0][1]>=0.25);
    assert_eq!(bounds.max_component_abs,[0.875]);
    for budget in 0..100-work {assert!(left.merged(right,&mut {budget},&||false).is_none());}
    let polls=std::cell::Cell::new(0);left.merged(right,&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(left.merged(right,&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
    assert_eq!(left.alpha,right.alpha);
}

#[test]
fn aggregate_integral_area_allows_moment_rounding_without_weakening_fragment_validation() {
    let envelope=IntegralEnvelope::<1>::new();let area=1.+f64::EPSILON;
    assert!(envelope.added(area,0.5,[0.5],&mut 100,&||false).is_none());
    let aggregate=envelope.added_aggregate(area,0.5,[0.5],&mut 100,&||false).unwrap();
    let bounds=aggregate.finish(area*0.5,[area*0.25],&mut 100,&||false).unwrap();
    assert!(bounds.alpha[0]<=area*0.5 && bounds.alpha[1]>=area*0.5);
    assert!(envelope.added_aggregate(1.+2e-12,0.5,[0.5],&mut 100,&||false).is_none());
}

#[test]
fn reference_intervals_cover_reported_products_and_preserve_compensated_values() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let mut bounded=MomentTable::<1,2>::admitted_prefix_with_reference_bounds(2,Point::ZERO,1,true,&admit).unwrap();
    let mut ordinary=MomentTable::<1,2>::admitted_prefix(2,Point::ZERO,1,&admit).unwrap();
    for i in 0..4096 {
        let area=1./4096.;let alpha=0.5;let component=(i as f64+0.5)/4096.;let reference=[alpha,alpha*component];let p=Point::new(component,0.5);
        bounded.add_sample([1],area,p,reference,&mut 1000,&||false).unwrap();ordinary.add_sample([1],area,p,reference,&mut 1000,&||false).unwrap();
    }
    assert_eq!(bounded.iter_with_reference().collect::<Vec<_>>(),ordinary.iter_with_reference().collect::<Vec<_>>());
    let intervals=bounded.reference_integral_bounds(0,&mut 100,&||false).unwrap();
    for (exact,[lo,hi]) in [0.5,0.25].into_iter().zip(intervals) {assert!(lo<=exact && exact<=hi);}
    let envelope=IntegralEnvelope::<1>::new().added_integral_bounds(intervals[0],[intervals[1]],[1.],&mut 100,&||false).unwrap();
    assert!(envelope.finish(0.5,[0.25],&mut 100,&||false).unwrap().component_error[0]<1e-10);
}
#[test]
fn reference_interval_insert_update_failures_preserve_all_states() {
    let admit=|_|Some(Box::new(()) as Box<dyn std::any::Any+Send+Sync>);
    let make=|| {let mut t=MomentTable::<1,1>::admitted_prefix_with_reference_bounds(2,Point::ZERO,1,true,&admit).unwrap();t.add_sample([1],0.25,Point::new(0.25,0.5),[0.5],&mut 1000,&||false).unwrap();t};
    for key in [1,2] {
        let mut probe=make();let mut work=1000;probe.add_sample([key],0.25,Point::new(0.75,0.5),[0.75],&mut work,&||false).unwrap();let cost=1000-work;
        let polls=std::cell::Cell::new(0);let mut probe=make();probe.add_sample([key],0.25,Point::new(0.75,0.5),[0.75],&mut 1000,&||{polls.set(polls.get()+1);false}).unwrap();
        for failure in 0..cost+polls.get() {
            let mut table=make();let before=table.iter_with_reference().collect::<Vec<_>>();let geometry=table.geometry_rounding_bounds(0,&mut 100,&||false).unwrap();let bounds=table.reference_integral_bounds(0,&mut 100,&||false).unwrap();let count=std::cell::Cell::new(0);
            let mut allowance=if failure<cost {failure} else {1000};let stop=if failure<cost {usize::MAX} else {failure-cost+1};
            assert!(table.add_sample([key],0.25,Point::new(0.75,0.5),[0.75],&mut allowance,&||{count.set(count.get()+1);count.get()>=stop}).is_none());
            assert_eq!(table.iter_with_reference().collect::<Vec<_>>(),before);assert_eq!(table.reference_integral_bounds(0,&mut 100,&||false),Some(bounds));assert_eq!(table.geometry_rounding_bounds(0,&mut 100,&||false),Some(geometry));assert_eq!(table.reference_bounds.as_ref().unwrap().len(),1);
        }
    }
}
#[test]
fn reference_interval_credit_denial_and_drop_release_every_allocation() {
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);impl Drop for Credit {fn drop(&mut self){self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    for denial in 1..=4 {
        let calls=std::cell::Cell::new(0);let admit=|bytes| {calls.set(calls.get()+1);if calls.get()==denial {return None;}used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
        let table=MomentTable::<1,1>::admitted_prefix_with_reference_bounds(2,Point::ZERO,1,true,&admit);
        assert_eq!(table.is_some(),denial==4);drop(table);assert_eq!(used.load(Ordering::Relaxed),0);
    }
}

#[test]
fn enclosed_reference_addition_refuses_invalid_intervals_work_and_every_cancel_checkpoint() {
    let envelope=IntegralEnvelope::<2>::new();let alpha=[0.25,0.25000000000001];let values=[[0.1,0.10000000000001],[0.2,0.20000000000001]];
    let mut work=100;envelope.added_integral_bounds(alpha,values,[1.;2],&mut work,&||false).unwrap();
    for budget in 0..100-work {assert!(envelope.added_integral_bounds(alpha,values,[1.;2],&mut {budget},&||false).is_none());}
    let polls=std::cell::Cell::new(0);envelope.added_integral_bounds(alpha,values,[1.;2],&mut 100,&||{polls.set(polls.get()+1);false}).unwrap();
    for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(envelope.added_integral_bounds(alpha,values,[1.;2],&mut 100,&||{count.set(count.get()+1);count.get()>=stop}).is_none());}
    assert!(envelope.added_integral_bounds([1.,0.],values,[1.;2],&mut 100,&||false).is_none());
    assert!(envelope.added_integral_bounds(alpha,[[f64::NAN,0.],[0.,0.]],[1.;2],&mut 100,&||false).is_none());
    assert!(envelope.added_integral_bounds(alpha,values,[-1.;2],&mut 100,&||false).is_none());
}

#[test]
fn reference_input_padding_covers_upstream_product_residual() {
    for (a,b) in [(0.1f64,0.2f64),(1./3.,1./3.),(0.99,0.37),(0.25,-0.3)] {
        let rounded=a*b;
        // A fused residual supplies an independent two-product comparison;
        // the exact residual here is representable without adding it back.
        let residual=a.mul_add(b,-rounded);
        let bounds=accumulated_reference_bounds([[0.;2]],1.,[rounded],&mut 100,&||false).unwrap()[0];
        assert!(rounded-bounds[0]>=(-residual).max(0.));
        assert!(bounds[1]-rounded>=residual.max(0.));
    }
}
