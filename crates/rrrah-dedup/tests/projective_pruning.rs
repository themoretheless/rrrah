//! Compare against the complete pre-pruning fitter, not a duplicate new loop.
#[allow(dead_code)]
#[path="fixtures/projective-pruning-reference.rs"] mod reference;
use rrrah_dedup::geometry as candidate;
use std::cell::Cell;
fn compare(raw:&[[f64;4]],seed:u64,trials:u64,joint:bool) -> (usize,usize) {
    let a:Vec<_>=raw.iter().map(|p|candidate::Correspondence{source:[p[0],p[1]],target:[p[2],p[3]]}).collect();
    let b:Vec<_>=raw.iter().map(|p|reference::Correspondence{source:[p[0],p[1]],target:[p[2],p[3]]}).collect();
    let p=candidate::GeometryPolicy{tolerance:2.,min_inliers:10,max_points:a.len(),max_hypotheses:trials};
    let q=reference::GeometryPolicy{tolerance:p.tolerance,min_inliers:p.min_inliers,max_points:p.max_points,max_hypotheses:p.max_hypotheses};
    let sa=candidate::ProjectiveSamplingPolicy{trials,seed};let sb=reference::ProjectiveSamplingPolicy{trials,seed};
    let ca=Cell::new(0);let cb=Cell::new(0);
    let ac=||{ca.set(ca.get()+1);false};let bc=||{cb.set(cb.get()+1);false};
    if joint {
        let work=2*a.len() as u64*(trials+1);
        let a=candidate::verify_projective_sampled_joint_for_domains(&a,candidate::JointProjectivePolicy{target:p,source_tolerance:0.5,maximum_projection_checks:work},sa,(2048,2048),(2048,2048),ac).unwrap();
        let b=reference::verify_projective_sampled_joint_for_domains(&b,reference::JointProjectivePolicy{target:q,source_tolerance:0.5,maximum_projection_checks:work},sb,(2048,2048),(2048,2048),bc).unwrap();
        assert_eq!(a.as_ref().map(|e|(e.transform.matrix.map(|r|r.map(f64::to_bits)),&e.inliers,e.normalized_squared_error.to_bits(),e.hypotheses)),b.as_ref().map(|e|(e.transform.matrix.map(|r|r.map(f64::to_bits)),&e.inliers,e.normalized_squared_error.to_bits(),e.hypotheses)));
    } else {
        let a=candidate::verify_projective_sampled_for_domains(&a,p,sa,(2048,2048),(2048,2048),ac).unwrap();
        let b=reference::verify_projective_sampled_for_domains(&b,q,sb,(2048,2048),(2048,2048),bc).unwrap();
        assert_eq!(a.as_ref().map(|e|(e.transform.matrix.map(|r|r.map(f64::to_bits)),&e.inliers,e.squared_error.to_bits(),e.hypotheses)),b.as_ref().map(|e|(e.transform.matrix.map(|r|r.map(f64::to_bits)),&e.inliers,e.squared_error.to_bits(),e.hypotheses)));
    }
    (ca.get(),cb.get())
}
#[test]
fn seeded_identity_noise_and_outliers_match_unpruned_bits_and_exercise_dominance() {
    let mut reduced=false;
    for mode in 0..3 {
        let raw:Vec<_>=(0..80).map(|i|{let x=(i%10) as f64*13.+20.;let y=(i/10) as f64*17.+30.;let noise=if mode==0{0.}else{((i*17%13) as f64-6.)*0.02};let outlier=mode==2&&i%4==0;[x,y,if outlier{x+70.}else{x+4.+noise},if outlier{y-20.}else{y+8.-noise}]}).collect();
        for seed in 1..=12 {for joint in [false,true] {let(a,b)=compare(&raw,seed,512,joint);assert!(a<=b);if !joint{assert_eq!(a,b);}reduced|=a<b;}}
    }
    assert!(reduced,"reference comparisons must actually exercise dominated candidates");
}
mod data_204100 {include!("fixtures/joint-geometry-204100.bits.rs");pub fn raw()->Vec<[f64;4]>{POINTS.iter().map(|p|p.map(f64::from_bits)).collect()}}
mod data_211900 {include!("fixtures/joint-geometry-211900.bits.rs");pub fn raw()->Vec<[f64;4]>{POINTS.iter().map(|p|p.map(f64::from_bits)).collect()}}
#[test]
fn real_reverse_correspondences_match_complete_reference_in_both_scoring_modes() {
    for raw in [data_204100::raw(),data_211900::raw()] {for seed in [173,0x1234abcd] {for joint in [false,true] {compare(&raw,seed,4096,joint);}}}
}
#[test]
fn cancellation_and_malformed_suffix_still_refuse_before_any_evidence() {
    let mut points:Vec<_>=(0..20).map(|i|candidate::Correspondence{source:[(i%5) as f64,(i/5) as f64],target:[(i%5) as f64,(i/5) as f64]}).collect();
    let p=candidate::JointProjectivePolicy{target:candidate::GeometryPolicy{tolerance:2.,min_inliers:10,max_points:20,max_hypotheses:512},source_tolerance:0.5,maximum_projection_checks:40*513};let s=candidate::ProjectiveSamplingPolicy{trials:512,seed:173};
    for stop in [1,100,1000] {let count=Cell::new(0);let e=candidate::verify_projective_sampled_joint_for_domains(&points,p,s,(20,20),(20,20),||{count.set(count.get()+1);count.get()>=stop});assert!(matches!(e,Err(candidate::GeometryError::Cancelled)));}
    points.last_mut().unwrap().target[0]=f64::NAN;let e=candidate::verify_projective_sampled_joint_for_domains(&points,p,s,(20,20),(20,20),||false);assert!(matches!(e,Err(candidate::GeometryError::Invalid)));
}
