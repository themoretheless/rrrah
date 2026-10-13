#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def function(text,name):
 start=text.index('fn '+name+'(');brace=text.index('{',start);depth=1;i=brace+1
 while depth:
  depth += (text[i]=='{')-(text[i]=='}');i+=1
 return text[start:i]
def main():
 mp=D/'dedup-zero-smoothing-borrow-source-v2-20261012.json';m=json.loads(mp.read_text());parent=Path(m['snapshot']);s=T/'rrrah-zero-smoothing-borrow-candidate-v3-20261012';assert not s.exists();s.mkdir()
 for k,v in m['file_hashes'].items():
  assert sha(parent/k)==v,k
  dest=s/k;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(parent/k,dest)
 key='crates/rrrah-dedup/src/local_collection.rs';old=(s/key).read_text();text=old
 first='''        let samples = crate::gradient_scale::smooth_gradient_candidates_managed(
            view,
            config.smoothing_radius,
            config.max_smoothing_taps_per_image,
            budget,
            cancel,
        )?;'''
 replace='''        let samples = if config.smoothing_radius == 0 {
            crate::gradient_scale::validate_unsmoothed_gradient_candidates(
                view, config.max_smoothing_taps_per_image, cancel,
            )?;
            None
        } else {
            Some(crate::gradient_scale::smooth_gradient_candidates_managed(
                view, config.smoothing_radius, config.max_smoothing_taps_per_image, budget, cancel,
            )?)
        };'''
 assert text.count(first)==1;text=text.replace(first,replace)
 start=text.index('        let smooth = crate::linear::LinearRgbaView::new(',text.index('fn extract_candidate_union_proposals('));end=text.index('        extract(&smooth',start)
 block=text[start:end];block=block.replace('        let smooth =','        let smooth = if let Some(samples) = &samples {',1).replace('&samples,','samples,').replace('        })?;','        })?\n        } else { *view };')
 text=text[:start]+block+text[end:]
 first='''            let samples = crate::gradient_scale::smooth_gradient_candidates_managed(
                view,
                radius,
                config.max_smoothing_taps_per_image,
                budget,
                cancel,
            )?;'''
 replace='''            let samples = if radius == 0 {
                crate::gradient_scale::validate_unsmoothed_gradient_candidates(
                    view, config.max_smoothing_taps_per_image, cancel,
                )?;
                None
            } else {
                Some(crate::gradient_scale::smooth_gradient_candidates_managed(
                    view, radius, config.max_smoothing_taps_per_image, budget, cancel,
                )?)
            };'''
 assert text.count(first)==1;text=text.replace(first,replace)
 start=text.index('            let smooth =',text.index('fn extract_candidate_union_portfolio_proposals('));end=text.index('            crate::gradient_scale::extract_spatial',start)
 block=text[start:end].replace('            let smooth =','            let smooth = if let Some(samples) = &samples {',1).replace('&samples,','samples,').replace('                    })?;','                    })?\n            } else { *view };')
 text=text[:start]+block+text[end:]
 prefix=(s/'crates/rrrah-dedup/tests/candidate_union_files.rs').read_text().split('#[test]')[0].replace('#![cfg(feature = "decode")]','').replace('rrrah_dedup::','crate::')
 baseline=function(old,'extract_candidate_union_proposals').replace('extract_candidate_union_proposals','baseline_union')
 baseline2=function(old,'extract_candidate_union_portfolio_proposals').replace('extract_candidate_union_portfolio_proposals','baseline_portfolio').replace('extract_candidate_union_proposals','baseline_union')
 tests='''
    #[test]
    fn zero_collection_features_equal_owned_recipe_for_base_and_portfolio() {
        for (w,h) in [(32u32,32u32),(48,33),(33,48)] {
            let pixels: Vec<f32> = (0..w*h).flat_map(|i| {
                let x=i%w;let y=i/w;
                let v=((x*17+y*31+x*y*7)%251) as f32 / 125.;
                [v, v*0.8-0.2, v*1.3, 1.]
            }).collect();
            let view=crate::linear::LinearRgbaView::new(w,h,&pixels,u64::from(w*h),||false).unwrap();
            for radius in [0,2] {
                let mut p=config();p.smoothing_radius=radius;
                for radii in [&[][..], &[0][..], &[4,0][..]] {
                    let a=rrrah_core::MemoryBudget::new(64*1024*1024);
                    let b=rrrah_core::MemoryBudget::new(64*1024*1024);
                    let expected=baseline_portfolio(&view,p,radii,&factors(),&a,&||false).unwrap();
                    let actual=super::extract_candidate_union_portfolio_proposals(&view,p,radii,&factors(),&b,&||false).unwrap();
                    assert!(!actual.is_empty());assert_eq!(&*expected,&*actual,"{w}x{h},base={radius},radii={radii:?}");
                    drop((expected,actual));assert_eq!((a.used(),b.used()),(0,0));
                }
            }
        }
    }
    #[test]
    fn zero_collection_refusals_release_original_feature_credit() {
        let pixels=vec![1f32;32*32*4];
        let view=crate::linear::LinearRgbaView::new(32,32,&pixels,1024,||false).unwrap();
        let budget=rrrah_core::MemoryBudget::new(64*1024*1024);
        let mut p=config();p.smoothing_radius=0;p.max_smoothing_taps_per_image=2047;
        assert!(matches!(super::extract_candidate_union_proposals(&view,p,&factors(),&budget,&||false),Err(crate::local::LocalError::Budget)));
        assert_eq!(budget.used(),0);
        assert!(matches!(super::extract_candidate_union_proposals(&view,p,&factors(),&budget,&||true),Err(crate::local::LocalError::Cancelled)));
        assert_eq!(budget.used(),0);
        p.max_smoothing_taps_per_image=2048;
        drop(super::extract_candidate_union_proposals(&view,p,&factors(),&budget,&||false).unwrap());
        assert_eq!(budget.used(),0);
    }
'''
 text+='\n#[cfg(test)]\nmod zero_collection_candidate_tests {\nuse super::*;\n'+prefix+'\n'+baseline+'\n'+baseline2+'\n'+tests+'\n}\n';(s/key).write_text(text)
 hashes={k:sha(s/k)for k in m['file_hashes']};out=D/'dedup-zero-smoothing-borrow-source-v3-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'isolated_zero_borrow_pair_and_collection_candidate','snapshot':str(s),'parent_manifest':str(mp),'parent_sha256':sha(mp),'file_hashes':hashes,'deltas_from_v2':[k for k in hashes if hashes[k]!=m['file_hashes'][k]],'scope':'Production not changed. Collection zero-radius branches borrow validated original view. Private controls compare full descriptors against copied original functions across18 configurations; existing pair gate still required.'},indent=2)+'\n');print(s)
if __name__=='__main__':main()
