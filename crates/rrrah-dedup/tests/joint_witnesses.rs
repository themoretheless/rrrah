#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{geometry::{Correspondence,GeometryError,ProjectiveTransform},joint_witnesses::{JointWitnessPolicy,collect_joint_projective_witnesses as collect}};
use std::cell::Cell;
#[path="fixtures/joint-witnesses-200301.bits.rs"] mod real;
fn identity()->ProjectiveTransform {ProjectiveTransform{matrix:[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]}}
fn policy(n:usize)->JointWitnessPolicy {JointWitnessPolicy{source_tolerance:1.,target_tolerance:1.,minimum_separation:2.,maximum_points:n,maximum_projection_checks:2*n as u64,maximum_separation_checks:(n as u64)*(n.saturating_sub(1) as u64)/2}}
fn point(x:f64,y:f64)->Correspondence {Correspondence{source:[x,y],target:[x,y]}}
#[test]
fn real_cross_window_669_points_recover_exact_ten_witnesses() {
 let pts:Vec<_>=real::POINT_BITS.iter().map(|b|Correspondence{source:[f64::from_bits(b[0]),f64::from_bits(b[1])],target:[f64::from_bits(b[2]),f64::from_bits(b[3])]}).collect();
 let h=ProjectiveTransform{matrix:real::H_BITS.map(|r|r.map(f64::from_bits))};let mut p=policy(pts.len());p.source_tolerance=real::SOURCE_TOLERANCE;p.target_tolerance=2.;let budget=MemoryBudget::new(1_000_000);
 let out=collect(&pts,h,(692,349),(2048,1536),p,&budget,||false).unwrap();assert_eq!(out.len(),10);
 let indices=[85,263,264,266,268,269,271,289,291,301];
 for (a,&i) in out.iter().zip(&indices){assert_eq!(a.source.map(f64::to_bits),pts[i].source.map(f64::to_bits));assert_eq!(a.target.map(f64::to_bits),pts[i].target.map(f64::to_bits));}
 let held=out.clone();drop(out);assert!(budget.used()>0);drop(held);assert_eq!(budget.used(),0);
}
#[test]
fn rejects_both_endpoint_aliases_and_filters_both_residuals() {
 let mut pts=vec![point(10.,10.),point(11.,10.),point(30.,30.),point(50.,50.)];pts[2].source=[10.5,10.];pts[2].target=[11.,10.];pts[3].target=[60.,60.];let b=MemoryBudget::new(10000);
 let out=collect(&pts,identity(),(100,100),(100,100),policy(4),&b,||false).unwrap();assert_eq!(out.len(),1);drop(out);assert_eq!(b.used(),0);
}
#[test]
fn preflight_work_capacity_and_invalid_suffix_refuse_atomically() {
 let mut pts=vec![point(10.,10.),point(30.,30.)];let b=MemoryBudget::new(10000);
 for which in 0..3 {let mut p=policy(2);match which {0=>p.maximum_points=1,1=>p.maximum_projection_checks=3,_=>p.maximum_separation_checks=0};assert!(matches!(collect(&pts,identity(),(100,100),(100,100),p,&b,||false),Err(GeometryError::Budget)));assert_eq!(b.used(),0);}
 pts[1].target[0]=f64::NAN;assert!(matches!(collect(&pts,identity(),(100,100),(100,100),policy(2),&b,||false),Err(GeometryError::Invalid)));assert_eq!(b.used(),0);
 let tiny=MemoryBudget::new(1);assert!(matches!(collect(&[point(10.,10.)],identity(),(100,100),(100,100),policy(1),&tiny,||false),Err(GeometryError::Budget)));assert_eq!(tiny.used(),0);
}
#[test]
fn every_cancellation_checkpoint_releases_credit() {
 let pts=[point(10.,10.),point(30.,30.),point(50.,50.)];let b=MemoryBudget::new(10000);let calls=Cell::new(0);let out=collect(&pts,identity(),(100,100),(100,100),policy(3),&b,||{calls.set(calls.get()+1);false}).unwrap();drop(out);
 for at in 1..=calls.get(){let n=Cell::new(0);assert!(matches!(collect(&pts,identity(),(100,100),(100,100),policy(3),&b,||{n.set(n.get()+1);n.get()>=at}),Err(GeometryError::Cancelled)));assert_eq!(b.used(),0);}
}
#[test]
fn invalid_policy_domain_and_empty_input_do_not_prove_nonidentity() {
 let b=MemoryBudget::new(10000);for tolerance in [0.,f64::NAN,1e300,1e-300] {let mut p=policy(0);p.source_tolerance=tolerance;assert!(matches!(collect(&[],identity(),(100,100),(100,100),p,&b,||false),Err(GeometryError::Invalid)));}
 assert!(matches!(collect(&[],identity(),(0,100),(100,100),policy(0),&b,||false),Err(GeometryError::Invalid)));
 let out=collect(&[],identity(),(100,100),(100,100),policy(0),&b,||false).unwrap();assert!(out.is_empty());drop(out);assert_eq!(b.used(),0);
}

fn bank_policy(n:usize,max_bank:usize,banks:usize,trials:u64)->rrrah_dedup::joint_witnesses::JointBankPolicy {
 use rrrah_dedup::geometry::{GeometryPolicy,JointProjectivePolicy,ProjectiveSamplingPolicy};
 let fit=2*max_bank as u64*(trials+1);let w=policy(n);
 rrrah_dedup::joint_witnesses::JointBankPolicy{hypothesis:JointProjectivePolicy{target:GeometryPolicy{tolerance:1.,min_inliers:4,max_points:max_bank,max_hypotheses:trials},source_tolerance:1.,maximum_projection_checks:fit},sampling:ProjectiveSamplingPolicy{trials,seed:173},witnesses:w,minimum_witnesses:5,maximum_banks:banks,maximum_total_projection_checks:banks as u64*(fit+2*n as u64),maximum_total_separation_checks:banks as u64*w.maximum_separation_checks}
}
#[test]
fn real_bank_search_generates_h_without_supplied_transform() {
 use rrrah_dedup::joint_witnesses::search_joint_bank_candidate;
 let pts:Vec<_>=real::POINT_BITS.iter().map(|b|Correspondence{source:[f64::from_bits(b[0]),f64::from_bits(b[1])],target:[f64::from_bits(b[2]),f64::from_bits(b[3])]}).collect();
 let lengths=[65,56,57,35,32,18,56,34,18,12,35,21,42,26,23,11,8,15,12,16,10,8,18,19,22];
 let mut start=0;let banks:Vec<_>=lengths.iter().map(|&n|{let s=&pts[start..start+n];start+=n;s}).collect();assert_eq!(start,pts.len());
 let mut p=bank_policy(669,65,25,1_000_000);p.minimum_witnesses=10;p.hypothesis.source_tolerance=real::SOURCE_TOLERANCE;p.hypothesis.target.tolerance=2.;p.witnesses.source_tolerance=real::SOURCE_TOLERANCE;p.witnesses.target_tolerance=2.;
 let b=MemoryBudget::new(1_000_000);let result=search_joint_bank_candidate(&banks,(692,349),(2048,1536),p,&b,||false).unwrap().unwrap();assert_eq!(result.hypothesis_bank,6);assert_eq!(result.witnesses.len(),10);assert_eq!(result.transform.matrix.map(|r|r.map(f64::to_bits)),real::H_BITS);drop(result);assert_eq!(b.used(),0);
}
#[test]
fn bank_preflight_and_invalid_suffix_refuse_before_returning_candidate() {
 use rrrah_dedup::joint_witnesses::search_joint_bank_candidate as search;
 let first=[point(10.,10.),point(30.,10.),point(10.,30.),point(30.,30.)];let second=[point(50.,50.)];let banks=[first.as_slice(),second.as_slice()];let b=MemoryBudget::new(10000);let p=bank_policy(5,4,2,10);
 let result=search(&banks,(100,100),(100,100),p,&b,||false).unwrap().unwrap();assert_eq!(result.witnesses.len(),5);drop(result);assert_eq!(b.used(),0);
 for which in 0..3 {let mut p=p;match which{0=>p.maximum_total_projection_checks-=1,1=>p.maximum_total_separation_checks-=1,_=>p.maximum_banks=1};assert!(matches!(search(&banks,(100,100),(100,100),p,&b,||false),Err(GeometryError::Budget)));assert_eq!(b.used(),0);}
 let malformed=[point(f64::NAN,50.)];assert!(matches!(search(&[first.as_slice(),malformed.as_slice()],(100,100),(100,100),p,&b,||false),Err(GeometryError::Invalid)));assert_eq!(b.used(),0);
}
#[test]
fn bank_cancellation_is_atomic_and_output_ownership_is_managed() {
 use rrrah_dedup::joint_witnesses::search_joint_bank_candidate as search;
 let first=[point(10.,10.),point(30.,10.),point(10.,30.),point(30.,30.)];let second=[point(50.,50.)];let banks=[first.as_slice(),second.as_slice()];let b=MemoryBudget::new(10000);let p=bank_policy(5,4,2,2);let calls=Cell::new(0);
 let result=search(&banks,(100,100),(100,100),p,&b,||{calls.set(calls.get()+1);false}).unwrap().unwrap();let held=result.clone();drop(result);assert!(b.used()>0);drop(held);assert_eq!(b.used(),0);
 for at in 1..=calls.get(){let n=Cell::new(0);assert!(matches!(search(&banks,(100,100),(100,100),p,&b,||{n.set(n.get()+1);n.get()>=at}),Err(GeometryError::Cancelled)));assert_eq!(b.used(),0);}
}

#[test]
fn rejected_larger_candidate_does_not_prune_smaller_confirmed_candidate() {
 use rrrah_dedup::joint_witnesses::search_joint_bank_candidate_with as search;
 let first=[point(10.,10.),point(30.,10.),point(10.,30.),point(30.,30.)];
 let mut second=[point(60.,60.),point(80.,60.),point(60.,80.),point(80.,80.)];for p in &mut second {p.target[0]+=10.;}
 let extra=[point(50.,50.)];let banks=[first.as_slice(),second.as_slice(),extra.as_slice()];let mut p=bank_policy(9,4,3,100);p.minimum_witnesses=4;let b=MemoryBudget::new(10000);let counts=std::cell::RefCell::new(Vec::new());
 let result=search(&banks,(200,200),(200,200),p,&b,||false,|candidate|{counts.borrow_mut().push(candidate.witnesses.len());Ok::<_,GeometryError>(candidate.hypothesis_bank==1)}).unwrap().unwrap();assert_eq!(*counts.borrow(),vec![5,4]);assert_eq!(result.hypothesis_bank,1);assert_eq!(result.witnesses.len(),4);drop(result);assert_eq!(b.used(),0);
}
#[test]
fn candidate_qualifier_errors_are_atomic_and_callback_cancellation_wins() {
 use rrrah_dedup::joint_witnesses::search_joint_bank_candidate_with as search;
 let pts=[point(10.,10.),point(30.,10.),point(10.,30.),point(30.,30.)];let banks=[pts.as_slice()];let mut p=bank_policy(4,4,1,10);p.minimum_witnesses=4;let b=MemoryBudget::new(10000);
 assert!(matches!(search(&banks,(100,100),(100,100),p,&b,||false,|_|Err::<bool,_>(GeometryError::Budget)),Err(GeometryError::Budget)));assert_eq!(b.used(),0);
 let stop=Cell::new(false);assert!(matches!(search(&banks,(100,100),(100,100),p,&b,||stop.get(),|_|{stop.set(true);Err::<bool,_>(GeometryError::Invalid)}),Err(GeometryError::Cancelled)));assert_eq!(b.used(),0);
}
