//! Admitted owner-keyed geometry moments; no source colors are stored or read.
//! Ownership/source/route certification belongs to the caller. Bounds cover
//! reported fragment inputs and arithmetic, not original polygon clipping.
use kurbo::Point;
use std::mem::size_of;
use super::super::native_moments::positive_unit_stream_mass_bounds;
const EMPTY:u16=u16::MAX;
const MAX_OWNER:usize=1_048_575;
type Credit=Box<dyn std::any::Any+Send+Sync>;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) struct Binding {pub(super) command:usize,pub(super) owner:usize,pub(super) slot:usize}
#[derive(Clone,Copy,Debug)]
pub(super) struct Group {
    pub(super) binding:Binding,pub(super) count:usize,pub(super) area:f64,
    pub(super) centroid:Point,pub(super) area_bounds:[f64;2],
    pub(super) centroid_bounds:[[f64;2];2],
}
struct Entry {binding:Binding,count:usize,mass:[f64;3]}
pub(super) struct OwnerMoments {
    maps:Vec<Option<Vec<u16>>>,entries:Vec<Entry>,origin:Point,_credit:Credit,
}
fn debit(work:&mut usize,n:usize,cancelled:&dyn Fn()->bool)->Option<()> {
    if cancelled() {return None;}*work=work.checked_sub(n)?;Some(())
}
impl OwnerMoments {
    /// `work` is the caller's continuing geometry allowance, including setup.
    pub(super) fn admitted(command_count:usize,bindings:&[(usize,usize)],origin:Point,
        work:&mut usize,cancelled:&dyn Fn()->bool,admit:&dyn Fn(usize)->Option<Credit>)->Option<Self> {
        debit(work,1,cancelled)?;
        if command_count>4096 || bindings.len()>4095 || [origin.x,origin.y].iter()
            .any(|v|!v.is_finite() || v.abs()>1_048_576. || v.fract()!=0.) {return None;}
        let _scratch_credit=admit(command_count.checked_mul(size_of::<usize>())?)?;
        let mut sizes=Vec::new();sizes.try_reserve_exact(command_count).ok()?;
        if sizes.capacity()>command_count {return None;}
        debit(work,command_count,cancelled)?;sizes.resize(command_count,0usize);
        for &(command,owner) in bindings {
            debit(work,1,cancelled)?;
            if command>=command_count || owner>MAX_OWNER {return None;}
            sizes[command]=sizes[command].max(owner.checked_add(1)?);
        }
        debit(work,command_count,cancelled)?;
        let values=sizes.iter().try_fold(0usize,|sum,&n|sum.checked_add(n))?;
        let bytes=size_of::<Self>().checked_add(command_count.checked_mul(size_of::<Option<Vec<u16>>>())?)?
            .checked_add(values.checked_mul(size_of::<u16>())?)?
            .checked_add(bindings.len().checked_mul(size_of::<Entry>())?)?;
        // Dense initialization is prepaid, never hidden outside the work cap.
        debit(work,values.checked_add(command_count.checked_mul(2)?)?.checked_add(bindings.len())?,cancelled)?;
        let credit=admit(bytes)?;
        let mut maps=Vec::new();maps.try_reserve_exact(command_count).ok()?;
        if maps.capacity()>command_count {return None;}maps.resize_with(command_count,||None);
        for (command,size) in sizes.into_iter().enumerate() {
            if cancelled() {return None;}if size==0 {continue;}
            let mut map=Vec::new();map.try_reserve_exact(size).ok()?;
            if map.capacity()>size {return None;}map.resize(size,EMPTY);maps[command]=Some(map);
        }
        let mut entries=Vec::new();entries.try_reserve_exact(bindings.len()).ok()?;
        if entries.capacity()>bindings.len() {return None;}
        for &(command,owner) in bindings {
            if cancelled() {return None;}let slot=&mut maps[command].as_mut()?[owner];
            if *slot!=EMPTY {continue;}*slot=u16::try_from(entries.len()).ok()?;
            entries.push(Entry {binding:Binding {command,owner,slot:usize::from(*slot)},count:0,mass:[0.;3]});
        }
        if cancelled() {return None;}Some(Self {maps,entries,origin,_credit:credit})
    }
    pub(super) fn binding_count(&self)->usize {self.entries.len()}
    /// Validator consumes the same allowance. External validator state belongs
    /// to the caller and must be discarded with a refused traversal; entry
    /// moments commit only after validation and cancellation checks.
    pub(super) fn add(&mut self,command:usize,owner:usize,area:f64,point:Point,
        work:&mut usize,cancelled:&dyn Fn()->bool,
        mut validate:Option<&mut dyn FnMut(Binding,Point,&mut usize)->Option<()>>)->Option<()> {
        debit(work,16,cancelled)?;
        if !area.is_finite() || area<=0. || area>1. || !point.x.is_finite() || !point.y.is_finite()
            || point.x<self.origin.x || point.x>self.origin.x+1.
            || point.y<self.origin.y || point.y>self.origin.y+1. {return None;}
        let local=point-self.origin;
        if !(0. ..=1.).contains(&local.x) || !(0. ..=1.).contains(&local.y) {return None;}
        let slot=*self.maps.get(command)?.as_ref()?.get(owner)?;
        let entry=self.entries.get_mut(usize::from(slot))?;
        let count=entry.count.checked_add(1)?;
        let mass=[entry.mass[0]+area,entry.mass[1]+area*local.x,entry.mass[2]+area*local.y];
        if mass.iter().any(|v|!v.is_finite()) || cancelled() {return None;}
        if let Some(validate)=validate.as_mut() {
            let before=*work;validate(entry.binding,point,work)?;if *work>before {*work=before;return None;}
        }
        if cancelled() {return None;}entry.count=count;entry.mass=mass;Some(())
    }
    pub(super) fn visit_bindings(&self,work:&mut usize,cancelled:&dyn Fn()->bool,
        visit:&mut dyn FnMut(Binding,&mut usize)->Option<()>)->Option<()> {
        for entry in &self.entries {
            debit(work,1,cancelled)?;let before=*work;visit(entry.binding,work)?;
            if *work>before {*work=before;return None;}if cancelled() {return None;}
        }Some(())
    }
    /// Visitor refusal discards the entire export; earlier callback results must
    /// not be published as a complete pixel. Source colors remain caller-owned.
    pub(super) fn visit_groups(&self,work:&mut usize,cancelled:&dyn Fn()->bool,
        visit:&mut dyn FnMut(Group,&mut usize)->Option<()>)->Option<()> {
        for entry in &self.entries {
            debit(work,1,cancelled)?;if entry.count==0 {continue;}
            let area=positive_unit_stream_mass_bounds(entry.count,0,entry.mass[0],work,cancelled)?;
            if area[0]<=0. {return None;}
            let x=positive_unit_stream_mass_bounds(entry.count,1,entry.mass[1],work,cancelled)?;
            let y=positive_unit_stream_mass_bounds(entry.count,1,entry.mass[2],work,cancelled)?;
            debit(work,40,cancelled)?;
            let centroid=Point::new(self.origin.x+entry.mass[1]/entry.mass[0],self.origin.y+entry.mass[2]/entry.mass[0]);
            // Local subtraction can round near a pixel crossing zero. Include
            // that error before recovering the original global-point centroid.
            let coordinate_error=(f64::EPSILON+f64::from_bits(1)).next_up();
            let bounds=|mass:[f64;2],origin:f64| {
                let lo=((mass[0]/area[1]).next_down()-coordinate_error).next_down().max(0.);
                let hi=((mass[1]/area[0]).next_up()+coordinate_error).next_up().min(1.);
                [(origin+lo).next_down(),(origin+hi).next_up()]
            };
            let centroid_bounds=[bounds(x,self.origin.x),bounds(y,self.origin.y)];
            if [centroid.x,centroid.y].iter().any(|v|!v.is_finite())
                || centroid_bounds.iter().any(|[lo,hi]|!lo.is_finite() || !hi.is_finite() || lo>hi) || cancelled() {return None;}
            let before=*work;visit(Group {binding:entry.binding,count:entry.count,area:entry.mass[0],
                centroid,area_bounds:area,centroid_bounds},work)?;
            if *work>before {*work=before;return None;}if cancelled() {return None;}
        }Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Guard(Arc<AtomicUsize>,usize);
    impl Drop for Guard {fn drop(&mut self) {self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    fn collect(moments:&OwnerMoments)->Vec<Group> {
        let mut groups=Vec::new();moments.visit_groups(&mut 100_000,&||false,
            &mut |g,_| {groups.push(g);Some(())}).unwrap();groups
    }
    #[test]
    fn owner_groups_preserve_namespaces_aliases_and_independent_dyadic_moments() {
        let admit=|_|Some(Box::new(()) as Credit);
        let mut moments=OwnerMoments::admitted(2,&[(0,100),(0,100),(0,7),(1,100)],Point::ZERO,
            &mut 100_000,&||false,&admit).unwrap();assert_eq!(moments.binding_count(),3);
        let mut bindings=Vec::new();moments.visit_bindings(&mut 100,&||false,
            &mut |b,_| {bindings.push(b);Some(())}).unwrap();assert_eq!(bindings.len(),3);
        for i in 0..1024 {
            moments.add(0,100,1./1024.,Point::new((i as f64+0.5)/1024.,0.5),
                &mut 100,&||false,Some(&mut |b,_,work| {
                    assert_eq!(b.command,0);assert_eq!(b.owner,100);assert_eq!(b.slot,0);
                    *work=work.checked_sub(3)?;Some(())
                })).unwrap();
        }
        moments.add(1,100,0.25,Point::new(0.25,0.75),&mut 100,&||false,None).unwrap();
        let groups=collect(&moments);assert_eq!(groups.len(),2);
        let g=groups[0];assert_eq!(g.count,1024);assert_eq!(g.area,1.);assert_eq!(g.centroid,Point::new(0.5,0.5));
        assert!(g.area_bounds[0]<=1. && g.area_bounds[1]>=1.);
        for bounds in g.centroid_bounds {assert!(bounds[0]<=0.5 && bounds[1]>=0.5);}
        assert_eq!(groups[1].binding.command,1);assert_eq!(groups[1].binding.owner,100);
        assert_eq!(groups[1].area,0.25);assert_eq!(groups[1].centroid,Point::new(0.25,0.75));
        let mut crossing=OwnerMoments::admitted(1,&[(0,0)],Point::new(-1.,-1.),&mut 100,&||false,&admit).unwrap();
        crossing.add(0,0,1.,Point::new(-1e-30,-1e-30),&mut 100,&||false,None).unwrap();
        for bounds in collect(&crossing)[0].centroid_bounds {assert!(bounds[0]<=-1e-30 && bounds[1]>=-1e-30);}
        assert!(crossing.add(0,0,0.5,Point::new(f64::from_bits(1),-0.5),&mut 100,&||false,None).is_none());
    }
    #[test]
    fn owner_moment_refusals_preserve_entries_and_bound_source_indices() {
        let admit=|_|Some(Box::new(()) as Credit);
        let mut m=OwnerMoments::admitted(1,&[(0,7)],Point::ZERO,&mut 1000,&||false,&admit).unwrap();
        m.add(0,7,0.5,Point::new(0.25,0.5),&mut 100,&||false,None).unwrap();
        let saved=(m.entries[0].count,m.entries[0].mass.map(f64::to_bits));
        for budget in 0..16 {assert!(m.add(0,7,0.5,Point::new(0.75,0.5),&mut {budget},&||false,None).is_none());}
        for (command,owner,area,point) in [(1,7,0.5,Point::ZERO),(0,8,0.5,Point::ZERO),
            (0,7,0.,Point::ZERO),(0,7,1.1,Point::ZERO),(0,7,f64::NAN,Point::ZERO),
            (0,7,0.5,Point::new(1.1,0.5)),(0,7,0.5,Point::new(f64::NAN,0.))] {
            assert!(m.add(command,owner,area,point,&mut 100,&||false,None).is_none());
            assert_eq!((m.entries[0].count,m.entries[0].mass.map(f64::to_bits)),saved);
        }
        assert!(m.add(0,7,0.5,Point::ZERO,&mut 100,&||false,Some(&mut |_,_,_|None)).is_none());
        assert!(m.add(0,7,0.5,Point::ZERO,&mut 100,&||false,Some(&mut |_,_,work| {*work+=1;Some(())})).is_none());
        assert_eq!((m.entries[0].count,m.entries[0].mass.map(f64::to_bits)),saved);
        let cancelled=std::cell::Cell::new(false);
        assert!(m.add(0,7,0.5,Point::ZERO,&mut 100,&||cancelled.get(),
            Some(&mut |_,_,_| {cancelled.set(true);Some(())})).is_none());
        assert_eq!((m.entries[0].count,m.entries[0].mass.map(f64::to_bits)),saved);
        let polls=std::cell::Cell::new(0usize);
        m.add(0,7,0.5,Point::new(0.75,0.5),&mut 100,&||{polls.set(polls.get()+1);false},None).unwrap();
        for stop in 1..=polls.get() {
            let mut trial=OwnerMoments::admitted(1,&[(0,7)],Point::ZERO,&mut 1000,&||false,&admit).unwrap();
            let n=std::cell::Cell::new(0usize);
            assert!(trial.add(0,7,0.5,Point::ZERO,&mut 100,&||{n.set(n.get()+1);n.get()>=stop},None).is_none());
            assert_eq!(trial.entries[0].count,0);
        }
        assert!(OwnerMoments::admitted(1,&[(0,MAX_OWNER+1)],Point::ZERO,&mut 100,&||false,&admit).is_none());
        assert!(OwnerMoments::admitted(1,&[(0,0)],Point::new(0.5,0.),&mut 100,&||false,&admit).is_none());
        let wide=OwnerMoments::admitted(1,&[(0,MAX_OWNER)],Point::ZERO,&mut 40_000_000,&||false,&admit).unwrap();
        assert_eq!(wide.binding_count(),1);
    }
    #[test]
    fn owner_moment_admission_and_setup_work_refusal_release_all_credit() {
        let used=Arc::new(AtomicUsize::new(0));
        let admit=|bytes| {used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Guard(used.clone(),bytes)) as Credit)};
        let bindings=[(0,7),(1,100)];let mut left=100_000;
        let moments=OwnerMoments::admitted(2,&bindings,Point::ZERO,&mut left,&||false,&admit).unwrap();
        assert!(used.load(Ordering::Relaxed)>0);drop(moments);assert_eq!(used.load(Ordering::Relaxed),0);
        for budget in 0..100_000-left {
            assert!(OwnerMoments::admitted(2,&bindings,Point::ZERO,&mut {budget},&||false,&admit).is_none());
            assert_eq!(used.load(Ordering::Relaxed),0);
        }
        for denied in 0..2 {
            let calls=std::cell::Cell::new(0usize);
            let selective=|bytes| {let n=calls.get();calls.set(n+1);if n==denied {None} else {admit(bytes)}};
            assert!(OwnerMoments::admitted(2,&bindings,Point::ZERO,&mut 100_000,&||false,&selective).is_none());
            assert_eq!(used.load(Ordering::Relaxed),0);
        }
        let polls=std::cell::Cell::new(0usize);
        drop(OwnerMoments::admitted(2,&bindings,Point::ZERO,&mut 100_000,
            &||{polls.set(polls.get()+1);false},&admit).unwrap());
        for stop in 1..=polls.get() {
            let n=std::cell::Cell::new(0usize);
            assert!(OwnerMoments::admitted(2,&bindings,Point::ZERO,&mut 100_000,
                &||{n.set(n.get()+1);n.get()>=stop},&admit).is_none());
            assert_eq!(used.load(Ordering::Relaxed),0);
        }
    }
    #[test]
    fn geometry_setup_callbacks_and_group_export_share_one_prepaid_limit() {
        use kurbo::Shape;
        use super::super::super::polygon_coverage::admitted_selected_polygon_row_budget;
        let path=kurbo::Rect::new(0.,0.,1.,1.).to_path(0.1);let regions=[(&path,vello_cpu::peniko::Fill::NonZero)];
        let admit=|_|Some(Box::new(()) as Credit);
        let run=|work:&mut usize| {
            let mut moments=OwnerMoments::admitted(1,&[(0,100)],Point::ZERO,work,&||false,&admit)?;
            let row=admitted_selected_polygon_row_budget(&regions,0,work,&||false,&admit)?;
            for rect in [kurbo::Rect::new(0.,0.,0.5,1.),kurbo::Rect::new(0.5,0.,1.,1.)] {
                row.pixel_rect_fragments_with_callback_budget(&regions,0,0,rect,work,&||false,&admit,1,19,
                    &mut |w,_|Some((w[0]!=0).then_some(0)),&mut |fragment,local| {
                        moments.add(0,100,fragment.area,fragment.centroid,local,&||false,
                            Some(&mut |b,_,remaining| {assert_eq!(b.slot,0);*remaining=remaining.checked_sub(3)?;Some(())}))?;
                        assert_eq!(*local,0);Some(())
                    })?;
            }
            let mut output=None;moments.visit_groups(work,&||false,&mut |g,_| {assert!(output.is_none());output=Some(g);Some(())})?;
            output
        };
        let mut remaining=40_000_000;let group=run(&mut remaining).unwrap();
        assert_eq!(group.count,2);assert_eq!(group.area,1.);assert_eq!(group.centroid,Point::new(0.5,0.5));
        let spent=40_000_000-remaining;let mut exact=spent;assert!(run(&mut exact).is_some());assert_eq!(exact,0);
        let mut short=spent-1;assert!(run(&mut short).is_none());assert!(short<spent);
        let mut tiny=OwnerMoments::admitted(1,&[(0,0)],Point::ZERO,&mut 100,&||false,&admit).unwrap();
        tiny.add(0,0,f64::from_bits(1),Point::new(0.5,0.5),&mut 100,&||false,None).unwrap();
        assert!(tiny.visit_groups(&mut 100_000,&||false,&mut |_,_|panic!("uncertified tiny mass")).is_none());
    }

}
