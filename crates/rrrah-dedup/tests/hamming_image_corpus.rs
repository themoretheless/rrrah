use rrrah_dedup::{Candidate, HammingIndex, IndexError};

// Derived 64-bit pHash observations from all 540 pinned public source41 dataset
// paths. Order/path/source pins live in near41-visual-corpus-prepared-20261015.json.
// This fixture tests hash-space recall, not visual duplicate ground truth.
fn hashes() -> Vec<u64> {
    include_str!("fixtures/near41-phash64.txt")
        .lines()
        .map(|line| u64::from_str_radix(line, 16).unwrap())
        .collect()
}

#[test]
fn image_hash_corpus_matches_full_scan_without_top_k_truncation() {
    let hashes = hashes();
    assert_eq!(hashes.len(), 540);
    let mut hamming8_directed_edges = 0;
    for order in 0..3 {
        let mut ids: Vec<usize> = (0..hashes.len()).collect();
        match order {
            1 => ids.reverse(),
            // 137 is coprime with 540: a deterministic permutation.
            2 => ids.sort_unstable_by_key(|&id| (id * 137) % hashes.len()),
            _ => {}
        }
        let mut index = HammingIndex::new();
        for &id in &ids {
            index
                .try_insert_with_cancel(id as u64, hashes[id], hashes.len(), || false)
                .unwrap();
        }
        // Reinsertion must remain a no-op when the entry budget is already full.
        for &id in &ids {
            index
                .try_insert_with_cancel(id as u64, hashes[id], hashes.len(), || false)
                .unwrap();
        }
        for (query_id, &query) in hashes.iter().enumerate() {
            for radius in [0, 1, 4, 8, 16, 32, 64, 65] {
                let mut expected: Vec<_> = hashes
                    .iter()
                    .enumerate()
                    .filter_map(|(id, &hash)| {
                        let distance = (hash ^ query).count_ones();
                        (distance <= radius.min(64)).then_some(Candidate {
                            id: id as u64,
                            distance,
                        })
                    })
                    .collect();
                expected.sort_unstable_by_key(|candidate| (candidate.distance, candidate.id));
                assert_eq!(
                    index.try_search_with_cancel(query, radius, hashes.len(), || false),
                    Ok(expected.clone())
                );
                // A result cap refuses the query; it never silently returns a prefix.
                assert_eq!(
                    index.try_search_with_cancel(query, radius, expected.len() - 1, || false),
                    Err(IndexError::Budget)
                );
                if order == 0 && radius == 8 {
                    hamming8_directed_edges += expected
                        .iter()
                        .filter(|candidate| candidate.id != query_id as u64)
                        .count();
                }
            }
        }
    }
    assert_eq!(hamming8_directed_edges, 2902);
}
