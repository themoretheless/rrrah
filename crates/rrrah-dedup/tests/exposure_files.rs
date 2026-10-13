#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{anchor_rank::AnchorRankPolicy, anchor_rank_file::{compare_anchor_rank_files,compare_exposure_anchor_rank_files,AnchorRankFileError}, geometry::{Correspondence,ProjectiveTransform},local_rank::{LocalRankPolicy,LocalRankError},rank_region::RankRegionPolicy,animated::AnimationBudget};
use std::{cell::Cell,sync::atomic::{AtomicU64,Ordering}};
fn model() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn points() -> Vec<Correspondence> {
    (0..10)
        .map(|i| {
            let p = [8. + f64::from(i % 5) * 8., 8. + f64::from(i / 5) * 16.];
            Correspondence { source: p, target: p }
        })
        .collect()
}
fn policy() -> AnchorRankPolicy {
    AnchorRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 2,
                minimum_contrast: 0.001,
                minimum_pairs: 100,
                maximum_sites: 980,
                maximum_pixel_reads: 4900,
            },
            filter_radius: 0,
            minimum_witnesses: 10,
            maximum_points: 10,
            maximum_point_checks: 55,
            minimum_point_separation: 2.,
            target_tolerance: 0.01,
            source_tolerance: 0.01,
            minimum_coverage: 0.3,
            minimum_agreement: 0.9,
        },
        window_radius: 1,
        maximum_selection_checks: 145,
    }
}

fn limits()->AnimationBudget{AnimationBudget{max_frames:1,max_pixels:4096,max_file_bytes:20000}}
struct Images {dir:std::path::PathBuf,left:rrrah_decode::DecodeRequest,right:rrrah_decode::DecodeRequest}
impl Drop for Images {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.dir);}}
fn images(inverted:bool)->Images{
 static NEXT:AtomicU64=AtomicU64::new(0);
 let dir=std::env::temp_dir().join(format!("rrrah-exposure-files-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir(&dir).unwrap();
 for (name,dark) in [("left.bmp",false),("right.bmp",true)]{
 let mut b=vec![0u8;54+64*64*3];let n=b.len()as u32;b[..2].copy_from_slice(b"BM");b[2..6].copy_from_slice(&n.to_le_bytes());b[10..14].copy_from_slice(&54u32.to_le_bytes());b[14..18].copy_from_slice(&40u32.to_le_bytes());b[18..22].copy_from_slice(&64i32.to_le_bytes());b[22..26].copy_from_slice(&64i32.to_le_bytes());b[26..28].copy_from_slice(&1u16.to_le_bytes());b[28..30].copy_from_slice(&24u16.to_le_bytes());
 for y in 0..64 {for x in 0..64 {let raw=20+((x*13+y*7)%29)*6;let v=if dark{if inverted{(220-raw)/16}else{raw/16}}else{raw};let i=54+((63-y)*64+x)*3;b[i..i+3].fill(v as u8);}}
 std::fs::write(dir.join(name),b).unwrap();}
 let mut left=rrrah_decode::DecodeRequest::new(dir.join("left.bmp"));left.assume_untagged_linear_srgb=true;let mut right=rrrah_decode::DecodeRequest::new(dir.join("right.bmp"));right.assume_untagged_linear_srgb=true;Images{dir,left,right}
}
#[test]
fn explicit_file_calibration_recovers_dark_rank_and_rejects_inversion(){
 for inverted in [false,true]{let f=images(inverted);let budget=MemoryBudget::new(2_000_000);let mut p=policy();p.local.rank.minimum_contrast=0.005;
 let raw=compare_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),p,&budget,||false).unwrap();assert!(!raw.supported);
 let e=compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),p,24576,&budget,||false).unwrap();assert_eq!(e.anchors.supported,!inverted);assert!(e.exposures[1].gain>e.exposures[0].gain);assert_eq!(e.exposures[0].pixel_reads,12288);assert_eq!(budget.used(),0);
 }}
#[test]
fn file_calibration_budget_overflow_and_cancel_precede_missing_sources(){
 let r=rrrah_decode::DecodeRequest::new("/nonexistent-rrrah-exposure-preflight.bmp");let budget=MemoryBudget::new(1000);
 for (cap,stop) in [(0,false),(0,true)]{let e=compare_exposure_anchor_rank_files(&r,&r,model(),&points(),limits(),policy(),cap,&budget,||stop).unwrap_err();assert!(if stop{matches!(e,AnchorRankFileError::Cancelled)}else{matches!(e,AnchorRankFileError::Rank(LocalRankError::Budget))});}
 let mut l=limits();l.max_pixels=u64::MAX;assert!(matches!(compare_exposure_anchor_rank_files(&r,&r,model(),&points(),l,policy(),u64::MAX,&budget,||false),Err(AnchorRankFileError::Rank(LocalRankError::Budget))));assert_eq!(budget.used(),0);
}
#[test]
fn late_file_calibration_cancellation_and_source_mutation_discard_evidence(){
 let f=images(false);let budget=MemoryBudget::new(2_000_000);let calls=Cell::new(0);
 compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&budget,||{calls.set(calls.get()+1);false}).unwrap();let total=calls.get();assert!(total>10);
 for stop in [1,total/2,total]{let n=Cell::new(0);assert!(compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&budget,||{n.set(n.get()+1);n.get()==stop}).is_err());assert_eq!(budget.used(),0);}
 let n=Cell::new(0);let result=compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&budget,||{n.set(n.get()+1);if n.get()==total{let mut b=std::fs::read(&f.right.path).unwrap();*b.last_mut().unwrap()^=1;std::fs::write(&f.right.path,b).unwrap();}false});assert!(matches!(result,Err(AnchorRankFileError::Source(_))));assert_eq!(budget.used(),0);
}

#[test]
fn calibrated_scratch_admission_fails_after_successful_legacy_decode_and_releases_credit(){
 let f=images(false);let raw=MemoryBudget::new(2_000_000);
 compare_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),&raw,||false).unwrap();
 let full=MemoryBudget::new(2_000_000);
 compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&full,||false).unwrap();
 assert!(full.peak()>raw.peak());
 let tight=MemoryBudget::new(raw.peak());
 compare_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),&tight,||false).unwrap();
 let result=compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&tight,||false);
 assert!(matches!(result,Err(AnchorRankFileError::Rank(LocalRankError::Budget))));
 assert_eq!(tight.used(),0);assert!(tight.peak()<=raw.peak());
}

#[test]
fn zero_exposure_file_is_explicitly_uninformative_and_releases_scratch(){
 let f=images(false);let mut bytes=std::fs::read(&f.right.path).unwrap();bytes[54..].fill(0);std::fs::write(&f.right.path,bytes).unwrap();
 let budget=MemoryBudget::new(2_000_000);
 assert!(matches!(compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&budget,||false),Err(AnchorRankFileError::Rank(LocalRankError::Rank(rrrah_dedup::rank_region::RankRegionError::Uninformative)))));
 assert_eq!(budget.used(),0);
}

#[test]
fn both_request_owners_cancel_mid_calibration_and_at_final_publication(){
 use std::sync::Arc;
 let f=images(false);let budget=MemoryBudget::new(2_000_000);let calls=Cell::new(0);
 compare_exposure_anchor_rank_files(&f.left,&f.right,model(),&points(),limits(),policy(),24576,&budget,||{calls.set(calls.get()+1);false}).unwrap();
 for stop in [calls.get()/2,calls.get()] {for owner in 0..2 {
 let generation=Arc::new(AtomicU64::new(1));let mut requests=[f.left.clone(),f.right.clone()];requests[owner].cancellation=Some(rrrah_decode::GenerationToken::new(generation.clone(),1));let n=Cell::new(0);
 let result=compare_exposure_anchor_rank_files(&requests[0],&requests[1],model(),&points(),limits(),policy(),24576,&budget,||{n.set(n.get()+1);if n.get()==stop{generation.store(2,Ordering::Relaxed);}false});
 assert!(matches!(result,Err(AnchorRankFileError::Cancelled)));assert_eq!(n.get(),stop);assert_eq!(budget.used(),0);
 }}
}
