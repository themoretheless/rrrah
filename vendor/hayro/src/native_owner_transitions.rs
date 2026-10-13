//! Geometry-only selected-owner state. Highest source index wins, including
//! aliased canonical regions and nonmonotonic source-index arrangements.
use super::SpatialCommand;
struct Word {region_word:usize,mask:u64,indices:[usize;64]}
struct State {words:Vec<Word>,positions:[usize;64],active:u64,monotonic:bool}
pub(super) struct OwnerTransitions {
    offsets:Vec<usize>,links:Vec<usize>,states:Vec<Option<State>>,
    _credit:Box<dyn std::any::Any+Send+Sync>,
}
fn step(work:&mut usize,cost:usize,cancelled:&dyn Fn()->bool)->Option<()> {
    if cancelled() {return None;}*work=work.checked_sub(cost)?;Some(())
}
impl OwnerTransitions {
    pub(super) fn admitted(commands:&[SpatialCommand<'_>],regions:usize,work:&mut usize,
        cancelled:&dyn Fn()->bool,admit:&dyn Fn(usize)->Option<Box<dyn std::any::Any+Send+Sync>>)->Option<Self> {
        if regions>4095 || commands.len()>4096 {return None;}
        let mut count=0usize;let mut word_count=0usize;
        for command in commands {
            step(work,65,cancelled)?;
            if let SpatialCommand::NativeSampledPaint {owners,..}=command {
                let mut masks=[0u64;64];
                for &(region,_) in *owners {
                    step(work,1,cancelled)?;if region>=regions {return None;}masks[region/64]|=1u64<<(region%64);
                }
                for mask in masks {step(work,1,cancelled)?;if mask!=0 {word_count=word_count.checked_add(1)?;count=count.checked_add(mask.count_ones() as usize)?;}}
                if count>16384 {return None;}
            }
        }
        let bytes=(regions+1).checked_mul(size_of::<usize>())?
            .checked_add(count.checked_mul(size_of::<usize>())?)?
            .checked_add(commands.len().checked_mul(size_of::<Option<State>>())?)?
            .checked_add(word_count.checked_mul(size_of::<Word>())?)?
            ;
        let credit=admit(bytes)?;
        step(work,(regions+1).checked_add(count)?.checked_add(commands.len().checked_mul(64)?)?,cancelled)?;
        let mut offsets=Vec::new();offsets.try_reserve_exact(regions+1).ok()?;offsets.resize(regions+1,0usize);
        let mut links=Vec::new();links.try_reserve_exact(count).ok()?;links.resize(count,0usize);
        let mut states=Vec::new();states.try_reserve_exact(commands.len()).ok()?;states.resize_with(commands.len(),||None);
        if offsets.capacity()>regions+1 || links.capacity()>count || states.capacity()>commands.len() {return None;}
        for (command,entry) in commands.iter().enumerate() {
            step(work,65,cancelled)?;
            if let SpatialCommand::NativeSampledPaint {owners,..}=entry {
                if owners.is_empty() {continue;}
                let mut masks=[0u64;64];
                for &(region,_) in *owners {step(work,1,cancelled)?;masks[region/64]|=1u64<<(region%64);}
                let n=masks.iter().filter(|&&m|m!=0).count();
                let mut words=Vec::new();words.try_reserve_exact(n).ok()?;
                if words.capacity()>n {return None;}
                let mut positions=[usize::MAX;64];
                for (word,mask) in masks.into_iter().enumerate() {
                    step(work,1,cancelled)?;
                    if mask!=0 {step(work,64,cancelled)?;positions[word]=words.len();words.push(Word {region_word:word,mask,indices:[0;64]});}
                }
                for &(region,owner) in *owners {
                    step(work,1,cancelled)?;let old=&mut words[positions[region/64]].indices[region%64];*old=(*old).max(owner);
                }
                let mut previous=None;let mut monotonic=true;
                for word in &words {
                    let mut bits=word.mask;
                    while bits!=0 {
                        step(work,1,cancelled)?;let bit=bits.trailing_zeros() as usize;bits&=bits-1;
                        let owner=word.indices[bit];if previous.is_some_and(|p|p>owner) {monotonic=false;}previous=Some(owner);
                        let region=word.region_word*64+bit;offsets[region+1]=offsets[region+1].checked_add(1)?;
                    }
                }
                states[command]=Some(State {words,positions,active:0,monotonic});
            }
        }
        for i in 1..=regions {step(work,1,cancelled)?;offsets[i]=offsets[i].checked_add(offsets[i-1])?;}
        let _scratch_credit=admit(regions.checked_mul(size_of::<usize>())?)?;
        step(work,regions,cancelled)?;
        let mut cursor=Vec::new();cursor.try_reserve_exact(regions).ok()?;cursor.extend_from_slice(&offsets[..regions]);
        if cursor.capacity()>regions {return None;}
        for (command,state) in states.iter().enumerate() {
            step(work,1,cancelled)?;
            if let Some(state)=state {for word in &state.words {
                let mut bits=word.mask;
                while bits!=0 {
                    step(work,1,cancelled)?;let bit=bits.trailing_zeros() as usize;bits&=bits-1;
                    let region=word.region_word*64+bit;links[cursor[region]]=command;cursor[region]+=1;
                }
            }}
        }
        Some(Self {offsets,links,states,_credit:credit})
    }
    pub(super) fn update(&mut self,region:usize,word_bits:u64,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        let start=*self.offsets.get(region)?;let end=*self.offsets.get(region.checked_add(1)?)?;
        let word=region/64;
        // Caller supplies the authoritative word after this one region's
        // transition. All other changed regions must also be reported.
        step(work,1usize.checked_add(end.checked_sub(start)?.checked_mul(2)?)?,cancelled)?;
        for &command in &self.links[start..end] {
            let state=self.states[command].as_mut()?;let entry=&state.words[state.positions[word]];
            if word_bits&entry.mask!=0 {state.active|=1u64<<word;} else {state.active&=!(1u64<<word);}
        }
        Some(())
    }
    pub(super) fn rebuild(&mut self,coverage:&[u64;64],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<()> {
        let words=self.states.iter().flatten().try_fold(0usize,|n,s|n.checked_add(s.words.len()))?;
        step(work,self.states.len().checked_mul(2)?.checked_add(words.checked_mul(2)?)?,cancelled)?;
        for state in self.states.iter_mut().flatten() {
            state.active=0;
            for word in &state.words {if coverage[word.region_word]&word.mask!=0 {state.active|=1u64<<word.region_word;}}
        }
        Some(())
    }
    /// Coverage must match all reported transitions (or the latest rebuild).
    pub(super) fn owner(&self,command:usize,coverage:&[u64;64],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Option<usize>> {
        step(work,1,cancelled)?;
        let Some(state)=self.states.get(command)?.as_ref() else {return Some(None);};
        if state.active==0 {return Some(None);}
        if state.monotonic {
            step(work,3,cancelled)?;
            let word=63-state.active.leading_zeros() as usize;let entry=&state.words[state.positions[word]];
            let covered=coverage[word]&entry.mask;if covered==0 {return None;}
            let bit=63-covered.leading_zeros() as usize;
            return Some(Some(entry.indices[bit]));
        }
        let mut needed=state.active;let mut selected=None;
        while needed!=0 {
            step(work,1,cancelled)?;let word=needed.trailing_zeros() as usize;needed&=needed-1;
            let entry=&state.words[state.positions[word]];let mut bits=coverage[word]&entry.mask;if bits==0 {return None;}
            while bits!=0 {
                step(work,1,cancelled)?;let bit=bits.trailing_zeros() as usize;bits&=bits-1;
                selected=Some(selected.map_or(entry.indices[bit],|old:usize|old.max(entry.indices[bit])));
            }
        }
        Some(selected)
    }
}

#[test]
fn owner_transitions_match_scan_for_aliases_and_nonmonotonic_source_indices() {
    use super::NativeField;
    use hayro_interpret::BlendMode;
    let a=[(0,0),(2,7),(2,50),(70,1000)];let b=[(0,100),(2,3),(70,4)];
    let sample=|_:kurbo::Point,_:&dyn Fn()->bool,_:&mut usize,_:Option<usize>|Some(None);
    let make=|owners|SpatialCommand::NativeSampledPaint {cells:None,field:NativeField::AffineOwner,regions:&[0],
        sample:&sample,owners,alpha_is_shape:false,mode:BlendMode::Normal};
    let commands=[make(&a),make(&b)];let mut work=1_000_000;
    let mut tracker=OwnerTransitions::admitted(&commands,128,&mut work,&||false,&|_|Some(Box::new(()))).unwrap();
    assert!(tracker.states[0].as_ref().unwrap().monotonic);
    assert!(!tracker.states[1].as_ref().unwrap().monotonic);
    let mut coverage=[0u64;64];let mut random=19u64;
    for _ in 0..4096 {
        random=random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let region=(random>>32) as usize%128;let mask=1u64<<(region%64);coverage[region/64]^=mask;
        tracker.update(region,coverage[region/64],&mut work,&||false).unwrap();
        tracker.update(region,coverage[region/64],&mut work,&||false).unwrap();
        for (command,owners) in [(0,&a[..]),(1,&b[..])] {
            let expected=owners.iter().filter(|&&(r,_)|coverage[r/64]&(1u64<<(r%64))!=0).map(|&(_,owner)|owner).max();
            assert_eq!(tracker.owner(command,&coverage,&mut work,&||false),Some(expected));
        }
    }
    coverage=[0;64];coverage[0]=1;
    tracker.rebuild(&coverage,&mut work,&||false).unwrap();
    assert_eq!(tracker.owner(0,&coverage,&mut work,&||false),Some(Some(0)));
    assert_eq!(tracker.owner(1,&coverage,&mut work,&||false),Some(Some(100)));
    assert!(tracker.owner(0,&[0;64],&mut work,&||false).is_none());
    assert!(tracker.update(0,0,&mut 0,&||false).is_none());
    assert!(tracker.update(0,0,&mut work,&||true).is_none());
    assert!(tracker.rebuild(&[0;64],&mut 0,&||false).is_none());
    assert!(tracker.rebuild(&[0;64],&mut work,&||true).is_none());
    assert_eq!(tracker.owner(0,&coverage,&mut work,&||false),Some(Some(0)));
    coverage=[0;64];tracker.rebuild(&coverage,&mut work,&||false).unwrap();
    assert_eq!(tracker.owner(0,&coverage,&mut work,&||false),Some(None));
}

#[test]
fn owner_transition_preparation_cancellation_and_link_bound_release_credit() {
    use super::NativeField;
    use hayro_interpret::BlendMode;
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    struct Credit(Arc<AtomicUsize>,usize);
    impl Drop for Credit {fn drop(&mut self) {self.0.fetch_sub(self.1,Ordering::Relaxed);}}
    let used=Arc::new(AtomicUsize::new(0));
    let admit=|bytes|{used.fetch_add(bytes,Ordering::Relaxed);Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)};
    let owners=[(0,0),(2,7),(2,50),(70,1000)];
    let sample=|_:kurbo::Point,_:&dyn Fn()->bool,_:&mut usize,_:Option<usize>|Some(None);
    let make=|owners|SpatialCommand::NativeSampledPaint {cells:None,field:NativeField::AffineOwner,regions:&[0],
        sample:&sample,owners,alpha_is_shape:false,mode:BlendMode::Normal};
    let commands=[make(&owners)];let polls=std::cell::Cell::new(0);
    let tracker=OwnerTransitions::admitted(&commands,128,&mut 100000,&||{polls.set(polls.get()+1);false},&admit).unwrap();
    assert!(used.load(Ordering::Relaxed)>0);drop(tracker);assert_eq!(used.load(Ordering::Relaxed),0);
    for stop in 1..=polls.get() {
        let current=std::cell::Cell::new(0);
        assert!(OwnerTransitions::admitted(&commands,128,&mut 100000,&||{current.set(current.get()+1);current.get()==stop},&admit).is_none());
        assert_eq!(used.load(Ordering::Relaxed),0);
    }
    assert!(OwnerTransitions::admitted(&commands,128,&mut 0,&||false,&admit).is_none());
    assert!(OwnerTransitions::admitted(&commands,128,&mut 100000,&||false,&|_|None).is_none());
    assert!(OwnerTransitions::admitted(&commands,2,&mut 100000,&||false,&|_|panic!("invalid region must refuse before admission")).is_none());
    let many:Vec<_>=(0..64).map(|i|(i,i)).collect();let commands:Vec<_>=(0..257).map(|_|make(&many)).collect();let mut work=1_000_000;
    assert!(OwnerTransitions::admitted(&commands,128,&mut work,&||false,&|_|panic!("link ceiling must refuse before admission")).is_none());
    assert!(work>0);assert_eq!(used.load(Ordering::Relaxed),0);
}
