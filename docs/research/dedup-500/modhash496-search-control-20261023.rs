extern crate alloc;
#[path = "evidence/n24q02m--modhash/03-bktree.rs"]
mod original;
fn main() {
    let mut seed = 0x123456789abcdef0_u64;
    let mut entries = Vec::new();
    for id in 0..256_u64 {
        seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
        entries.push((seed, id));
    }
    entries.extend([(0, 9), (0, 1), (0, 1), (u64::MAX, 2)]);
    let mut checks = 0;
    for reverse in [false, true] {
        let mut tree = original::BkTree::new();
        assert!(tree.is_empty());
        assert!(tree.search(0, u32::MAX).is_empty());
        let mut inserted = entries.clone();
        if reverse { inserted.reverse(); }
        for &(hash, id) in &inserted { tree.insert(hash, id); }
        assert_eq!(tree.len(), entries.len());
        let queries = entries.iter().take(32).map(|x|x.0).chain([0,u64::MAX,0xaaaaaaaaaaaaaaaa]);
        for query in queries {
            for radius in [0,1,2,8,31,32,63,64,65,u32::MAX] {
                let actual: Vec<_> = tree.search(query,radius).iter().map(|c|(c.id,c.distance)).collect();
                let mut expected: Vec<_> = entries.iter().filter_map(|&(hash,id)|{
                    let distance=(hash^query).count_ones();
                    (distance<=radius).then_some((id,distance))
                }).collect();
                expected.sort_unstable();
                assert_eq!(actual,expected,"reverse={reverse}, radius={radius}, query={query}");
                checks+=1;
            }
        }
    }
    println!("{checks} search results match independent linear scan, including duplicate hashes/ids and oversized radii; ordering compared as actual id/distance contract, not documented distance/id.");
}
