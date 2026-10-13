//! Admitted per-region raster transitions. This tracks geometry identity only;
//! it neither samples color nor certifies floating-point integration.
use super::{RasterRegionKeys,SpatialCommand};

#[derive(Clone,Copy)]
struct Link {command:usize,mask:u64,alpha:bool}
struct State<'a> {cells:&'a RasterRegionKeys,color:u64,alpha:u64}
pub(super) struct RasterTransitions<'a> {
    offsets:Vec<usize>,links:Vec<Link>,states:Vec<Option<State<'a>>>,
    _credit:Box<dyn std::any::Any+Send+Sync>,
}

#[test]
fn transition_updates_match_independent_scan_and_refusals_preserve_state() {
    use super::NativeField;
    use hayro_interpret::BlendMode;
    let a=RasterRegionKeys {color:vec![(0,3),(2,5),(70,7)],alpha:vec![(1,11),(2,13)],alpha_present:true,_credit:Box::new(())};
    let b=RasterRegionKeys {color:vec![(0,103),(2,105),(70,107)],alpha:vec![],alpha_present:false,_credit:Box::new(())};
    let sample=|_:kurbo::Point,_:&dyn Fn()->bool,_:&mut usize,_:Option<usize>|Some(None);
    let make=|cells|SpatialCommand::NativeSampledPaint {cells:Some(cells),field:NativeField::ConstantCells,
        regions:&[0],sample:&sample,owners:&[],alpha_is_shape:false,mode:BlendMode::Normal};
    let commands=[make(&a),make(&b)];
    let mut work=1_000_000;
    let mut tracker=RasterTransitions::admitted(&commands,128,&mut work,&||false,&|_|Some(Box::new(()))).unwrap();
    let mut coverage=[0u64;64];let mut random=17u64;
    for _ in 0..4096 {
        random=random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let region=(random>>32) as usize%128;let mask=1u64<<(region%64);
        coverage[region/64]^=mask;
        tracker.update(region,coverage[region/64]&mask!=0,&mut work,&||false).unwrap();
        for (command,cells) in [(0,&a),(1,&b)] {
            assert_eq!(tracker.key(command,&mut work,&||false),cells.covered_key(&coverage,&mut 1000,&||false));
        }
    }
    coverage=[0;64];coverage[0]=1<<2;
    tracker.rebuild(&coverage,&mut work,&||false).unwrap();
    assert_eq!(tracker.key(0,&mut work,&||false),Some([5,13]));
    assert_eq!(tracker.key(1,&mut work,&||false),Some([105,u64::MAX]));
    assert!(tracker.update(2,false,&mut 0,&||false).is_none());
    assert!(tracker.update(2,false,&mut work,&||true).is_none());
    assert!(tracker.rebuild(&[0;64],&mut 0,&||false).is_none());
    assert!(tracker.rebuild(&[0;64],&mut work,&||true).is_none());
    assert_eq!(tracker.key(0,&mut work,&||false),Some([5,13]));
    assert_eq!(tracker.key(1,&mut work,&||false),Some([105,u64::MAX]));
}

#[test]
fn transition_admission_cancellation_and_bitmap_limit_release_all_credit() {
    use super::NativeField;
    use hayro_interpret::BlendMode;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self) {self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    let admit=|bytes| {used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    let cells=RasterRegionKeys {color:vec![(0,3),(2,5)],alpha:vec![(1,11),(2,13)],alpha_present:true,_credit:Box::new(())};
    let sample=|_:kurbo::Point,_:&dyn Fn()->bool,_:&mut usize,_:Option<usize>|Some(None);
    let make=|cells|SpatialCommand::NativeSampledPaint {cells:Some(cells),field:NativeField::ConstantCells,
        regions:&[0],sample:&sample,owners:&[],alpha_is_shape:false,mode:BlendMode::Normal};
    let commands=[make(&cells)];let polls=std::cell::Cell::new(0);
    let tracker=RasterTransitions::admitted(&commands,8,&mut 10000,&||{polls.set(polls.get()+1);false},&admit).unwrap();
    assert!(used.load(Ordering::Relaxed)>0);drop(tracker);assert_eq!(used.load(Ordering::Relaxed),0);
    for stop in 1..=polls.get() {
        let current=std::cell::Cell::new(0);
        assert!(RasterTransitions::admitted(&commands,8,&mut 10000,&||{current.set(current.get()+1);current.get()==stop},&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    assert!(RasterTransitions::admitted(&commands,8,&mut 0,&||false,&admit).is_none());
    assert!(RasterTransitions::admitted(&commands,8,&mut 10000,&||false,&|_|None).is_none());
    let dense=RasterRegionKeys {color:(0..65).map(|i|(i,i as u64)).collect(),alpha:vec![],alpha_present:false,_credit:Box::new(())};
    assert!(RasterTransitions::admitted(&[make(&dense)],128,&mut 10000,&||false,&admit).is_none());
    let wide=RasterRegionKeys {color:(0..64).map(|i|(i,i as u64)).collect(),alpha:vec![],alpha_present:false,_credit:Box::new(())};
    let too_many:Vec<_>=(0..257).map(|_|make(&wide)).collect();let mut work=100000;
    assert!(RasterTransitions::admitted(&too_many,128,&mut work,&||false,&|_|panic!("link ceiling must refuse before storage admission")).is_none());
    assert!(work>0);
    assert_eq!(used.load(Ordering::Relaxed),0);
}
fn step(work:&mut usize,cost:usize,cancelled:&dyn Fn()->bool)->Option<()> {
    if cancelled() {return None;}*work=work.checked_sub(cost)?;Some(())
}
impl<'a> RasterTransitions<'a> {
    pub(super) fn admitted(commands:&[SpatialCommand<'a>],regions:usize,work:&mut usize,
        cancelled:&dyn Fn()->bool,admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>)->Option<Self> {
        if regions>4095 || commands.len()>4096 {return None;}
        let mut count=0usize;
        for command in commands {
            step(work,1,cancelled)?;
            if let SpatialCommand::NativeSampledPaint {cells:Some(cells),..}=command {
                if cells.color.len()>64 || cells.alpha.len()>64 {return None;}
                for &(region,_) in cells.color.iter().chain(&cells.alpha) {
                    step(work,1,cancelled)?;if region>=regions {return None;}
                    count=count.checked_add(1)?;if count>16384 {return None;}
                }
            }
        }
        let bytes=(regions+1).checked_mul(size_of::<usize>())?
            .checked_add(count.checked_mul(size_of::<Link>())?)?
            .checked_add(commands.len().checked_mul(size_of::<Option<State<'a>>>())?)?;
        let credit=admit(bytes)?;
        step(work,regions.checked_add(1)?.checked_add(count)?.checked_add(commands.len())?,cancelled)?;
        let mut offsets=Vec::new();offsets.try_reserve_exact(regions+1).ok()?;offsets.resize(regions+1,0usize);
        let mut links=Vec::new();links.try_reserve_exact(count).ok()?;links.resize(count,Link {command:0,mask:0,alpha:false});
        let mut states=Vec::new();states.try_reserve_exact(commands.len()).ok()?;states.resize_with(commands.len(),||None);
        if offsets.capacity()>regions+1 || links.capacity()>count || states.capacity()>commands.len() {return None;}
        for (command,entry) in commands.iter().enumerate() {
            step(work,1,cancelled)?;
            if let SpatialCommand::NativeSampledPaint {cells:Some(cells),..}=entry {
                states[command]=Some(State {cells,color:0,alpha:0});
                for &(region,_) in cells.color.iter().chain(&cells.alpha) {
                    step(work,1,cancelled)?;offsets[region+1]=offsets[region+1].checked_add(1)?;
                }
            }
        }
        for i in 1..=regions {step(work,1,cancelled)?;offsets[i]=offsets[i].checked_add(offsets[i-1])?;}
        let _scratch_credit=admit(regions.checked_mul(size_of::<usize>())?)?;
        step(work,regions,cancelled)?;
        let mut cursor=Vec::new();cursor.try_reserve_exact(regions).ok()?;cursor.extend_from_slice(&offsets[..regions]);
        if cursor.capacity()>regions {return None;}
        for (command,state) in states.iter().enumerate() {
            step(work,1,cancelled)?;
            if let Some(state)=state {
                for (alpha,bindings) in [(false,&state.cells.color),(true,&state.cells.alpha)] {
                    for (bit,&(region,_)) in bindings.iter().enumerate() {
                        step(work,1,cancelled)?;
                        links[cursor[region]]=Link {command,mask:1u64<<bit,alpha};cursor[region]+=1;
                    }
                }
            }
        }
        Some(Self {offsets,links,states,_credit:credit})
    }
    pub(super) fn update(&mut self,region:usize,inside:bool,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        let start=*self.offsets.get(region)?;let end=*self.offsets.get(region.checked_add(1)?)?;
        // Debit the complete bounded transaction before committing any bit.
        step(work,1usize.checked_add(end.checked_sub(start)?)?,cancelled)?;
        for link in &self.links[start..end] {
            let state=self.states[link.command].as_mut()?;
            let bits=if link.alpha {&mut state.alpha} else {&mut state.color};
            if inside {*bits|=link.mask;} else {*bits&=!link.mask;}
        }
        Some(())
    }
    pub(super) fn rebuild(&mut self,coverage:&[u64;64],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        step(work,self.states.len().checked_add(self.links.len())?,cancelled)?;
        for state in self.states.iter_mut().flatten() {
            state.color=0;state.alpha=0;
            for (alpha,bindings) in [(false,&state.cells.color),(true,&state.cells.alpha)] {
                let bits=if alpha {&mut state.alpha} else {&mut state.color};
                for (bit,&(region,_)) in bindings.iter().enumerate() {
                    if coverage[region/64]&(1u64<<(region%64))!=0 {*bits|=1u64<<bit;}
                }
            }
        }
        Some(())
    }
    pub(super) fn key(&self,command:usize,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[u64;2]> {
        step(work,3,cancelled)?;
        let state=self.states.get(command)?.as_ref()?;
        if state.color.count_ones()!=1 {return None;}
        let color=state.cells.color[state.color.trailing_zeros() as usize].1;
        let alpha=if state.cells.alpha_present {
            if state.alpha.count_ones()!=1 {return None;}
            state.cells.alpha[state.alpha.trailing_zeros() as usize].1
        } else {u64::MAX};
        Some([color,alpha])
    }
}
