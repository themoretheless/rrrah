// Extracted unchanged pinned grouping bodies; synthetic entries and authored tests.
use std::collections::HashMap;
#[derive(Clone)] struct FileEntry { display_path:String,size:u64,mtime:u64 }
fn f(name:&str,size:u64)->FileEntry{FileEntry{display_path:name.into(),size,mtime:0}}
fn hamming_distance(a:u128,b:u128)->u32{(a^b).count_ones()}
fn cluster(hashes:Vec<Option<u128>>,threshold:u32)->Vec<Vec<usize>> { let n=hashes.len();
    let mut group_indices: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; n];

    for i in 0..n {
        if assigned[i] { continue; }
        let h1 = match hashes[i] {
            Some(h) => h,
            None => continue,
        };

        let mut current_group = vec![i];
        assigned[i] = true;

        for j in (i + 1)..n {
            if assigned[j] { continue; }
            let h2 = match hashes[j] {
                Some(h) => h,
                None => continue,
            };

            // Compare strictly to the representative image (h1) to prevent chaining
            if hamming_distance(h1, h2) <= threshold {
                current_group.push(j);
                assigned[j] = true;
            }
        }

        if current_group.len() > 1 {
            group_indices.push(current_group);
        }
    }

group_indices
}
fn find_duplicates(
    files: Vec<FileEntry>,
    mut hash_fn: impl FnMut(&FileEntry) -> Option<String>,
) -> (Vec<Vec<FileEntry>>, usize) {
    let mut size_groups: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    for f in files {
        size_groups.entry(f.size).or_default().push(f);
    }

    let candidates: Vec<Vec<FileEntry>> = size_groups
        .into_values()
        .filter(|v| v.len() > 1)
        .collect();

    let candidate_count: usize = candidates.iter().map(|v| v.len()).sum();
    println!(
        "Hashing {} files across {} same-size groups...",
        candidate_count,
        candidates.len()
    );

    let mut hash_groups: HashMap<String, Vec<FileEntry>> = HashMap::new();
    let mut errors = 0usize;

    for group in candidates {
        for entry in group {
            match hash_fn(&entry) {
                Some(hash) => hash_groups.entry(hash).or_default().push(entry),
                None => errors += 1,
            }
        }
    }

    let groups = hash_groups
        .into_values()
        .filter(|v| v.len() > 1)
        .collect();

    (groups, errors)
}

fn sort_dup_groups(groups: &mut Vec<Vec<FileEntry>>, strategy: &str) {
    for group in groups.iter_mut() {
        match strategy {
            "newest" => group.sort_by(|a, b| b.mtime.cmp(&a.mtime)),
            "oldest" => group.sort_by(|a, b| a.mtime.cmp(&b.mtime)),
            // Default: keep the largest file first (original vs compressed copies)
            _ => group.sort_by(|a, b| b.size.cmp(&a.size)),
        }
    }
    groups.sort_by(|a, b| a[0].display_path.cmp(&b[0].display_path));
}


#[test] fn representative_order_changes_partition(){
 assert_eq!(cluster(vec![Some(0),Some(1),Some(3)],1),vec![vec![0,1]]);
 assert_eq!(cluster(vec![Some(1),Some(0),Some(3)],1),vec![vec![0,1,2]]);
}
#[test] fn keeper_sort_can_replace_representative(){
 let mut groups=vec![vec![f("seed_hash_1",1),f("hash_0",3),f("hash_3",2)]];
 assert_eq!(cluster(vec![Some(1),Some(0),Some(3)],1)[0].len(),3);
 sort_dup_groups(&mut groups,"largest");
 assert_eq!(groups[0][0].display_path,"hash_0");assert_eq!(hamming_distance(0,3),2);
}
#[test] fn exact_groups_trust_injected_digest_without_confirmation(){
 let (groups,errors)=find_duplicates(vec![f("a",2),f("b",2)], |_|Some("same injected digest".into()));
 assert_eq!(groups.len(),1);assert_eq!(groups[0].len(),2);assert_eq!(errors,0);
}
#[test] fn hash_failure_keeps_healthy_pair(){
 let (groups,errors)=find_duplicates(vec![f("a",2),f("b",2),f("bad",2)], |f| if f.display_path=="bad"{None}else{Some("same".into())});
 assert_eq!(groups.len(),1);assert_eq!(groups[0].len(),2);assert_eq!(errors,1);
}
