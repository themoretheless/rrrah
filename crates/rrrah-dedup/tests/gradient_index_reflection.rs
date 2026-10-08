#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{gradient::{GradientDescriptor,GradientCellRecipe},gradient_index::GradientDescriptorIndex,local::LocalError};
#[test]
fn reflected_query_matches_independent_permutation_and_releases_index() {
    for recipe in [GradientCellRecipe::Fixed,GradientCellRecipe::Interpolated] {
        let mut q=[0.;128];q[19]=1.;let query=GradientDescriptor(q);
        // Independent canonical horizontal-reflection mapping: row flips and bin negates.
        let cell=19/8;let bin=19%8;let mirrored=((3-cell/4)*4+cell%4)*8+(8-bin)%8;
        let mut v=[0.;128];v[mirrored]=1.;let expected=GradientDescriptor(v);
        let entries=[(7,expected),(9,query)];let budget=MemoryBudget::new(16);
        let index=GradientDescriptorIndex::new(&entries,recipe,2,&budget,||false).unwrap();
        let result=index.search_reflected(&query,recipe,0.,2,2,128,||false).unwrap();
        assert_eq!(result,index.search(&expected,recipe,0.,2,2,||false).unwrap());
        assert_eq!(result.len(),1);assert_eq!(result[0].id,7);
        assert_eq!(budget.peak(),16);
        assert!(matches!(index.search_reflected(&query,recipe,0.,2,2,127,||false),Err(LocalError::Budget)));
        let calls=std::cell::Cell::new(0);
        index.search_reflected(&query,recipe,0.,2,2,128,||{calls.set(calls.get()+1);false}).unwrap();
        for stop in 1..=calls.get() {
            let seen=std::cell::Cell::new(0);
            assert!(matches!(index.search_reflected(&query,recipe,0.,2,2,128,||{seen.set(seen.get()+1);seen.get()==stop}),Err(LocalError::Cancelled)));
        }
        assert!(matches!(index.search_reflected(&query,recipe,0.,2,2,0,||true),Err(LocalError::Cancelled)));
        drop(index);assert_eq!(budget.used(),0);
    }
}

#[test]
fn reflected_pair_union_prepays_both_domains_and_keeps_all_hits() {
    use rrrah_dedup::gradient_index::{gradient_file_pair_report,gradient_file_pair_report_with_reflection};
    let mut a=[0.;128];a[0]=1.;let mut b=[0.;128];b[96]=1.;let mut c=[0.;128];c[1]=1.;
    let entries=[(0,GradientDescriptor(a)),(1,GradientDescriptor(b)),(2,GradientDescriptor(c))];
    let owners=[10,20,30];let recipe=GradientCellRecipe::Fixed;
    let budget=MemoryBudget::new(4096);
    let ordinary=gradient_file_pair_report(&entries,&owners,recipe,0.,9,20,3,&budget,||false).unwrap();assert!(ordinary.pairs.is_empty());drop(ordinary);
    let reflected=gradient_file_pair_report_with_reflection(&entries,&owners,recipe,0.,18,20,3,&budget,384,||false).unwrap();
    assert_eq!(reflected.pairs,[(10,20)]);assert_eq!(reflected.descriptor_hits,5);drop(reflected);assert_eq!(budget.used(),0);
    for (comparisons,bins) in [(17,384),(18,383)] {
        let fresh=MemoryBudget::new(4096);
        assert!(matches!(gradient_file_pair_report_with_reflection(&entries,&owners,recipe,0.,comparisons,20,3,&fresh,bins,||false),Err(LocalError::Budget)));
        assert_eq!(fresh.peak(),0);
    }
}
