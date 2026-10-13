#[cfg(test)]
mod rrrah_block_order_controls {
    use super::*;
    use tempfile::TempDir;
    fn files() -> (TempDir, Vec<FileInfo>) {
        let root = TempDir::new().unwrap();
        let p = vec![19u8; 4 * 4096];
        let a = vec![37u8; 4096];
        let b = vec![83u8; 4096];
        let left = [p.as_slice(), &a, &b, &p].concat();
        let right = [p.as_slice(), &b, &a, &p].concat();
        assert_ne!(left, right);
        let infos = [left.clone(), left, right].iter().enumerate().map(|(i, bytes)| {
            let path = root.path().join(format!("file-{i}.bin"));
            std::fs::write(&path, bytes).unwrap();
            FileInfo::new(path).unwrap()
        }).collect();
        (root, infos)
    }
    #[test]
    fn original_full_hash_ignores_block_order_for_five_seeds() {
        let (_root, infos) = files();
        for seed in [0, 1, -1, 300, i64::MAX] {
            assert_ne!(std::fs::read(&infos[0].path).unwrap(), std::fs::read(&infos[2].path).unwrap());
            assert_eq!(infos[0].initpages_hash(seed).unwrap(), infos[2].initpages_hash(seed).unwrap());
            let hashes: Vec<_> = infos.iter().map(|f| f.hash(seed).unwrap()).collect();
            assert_eq!(hashes, vec![hashes[0]; 3]);
            println!("seed={seed} full_hash={:032x} unequal_bytes=true",hashes[0]);
        }
    }
    #[test]
    fn original_strict_pipeline_groups_reordered_nonduplicate_with_true_copies() {
        let (_root, infos) = files();
        let input = Arc::new(Mutex::new(infos.clone()));
        let by_size = Arc::new(DashMap::new());
        Processor::sizewise(Arc::new(Params::default()), Arc::new(AtomicBool::new(true)), by_size.clone(), input, Arc::new(MultiProgress::new())).unwrap();
        let by_hash = Arc::new(DashMap::new());
        Processor::hashwise(Arc::new(Params { strict: true, ..Params::default() }), by_size, by_hash.clone(), Arc::new(MultiProgress::new()), Arc::new(AtomicU64::new(0)), 300, Arc::new(AtomicBool::new(true))).unwrap();
        assert_eq!(by_hash.len(), 1);
        let entry = by_hash.iter().next().unwrap();
        assert_eq!(entry.value().len(), 3);
        let mut actual: Vec<_> = entry.value().iter().map(|f| f.path.clone()).collect();
        let mut expected: Vec<_> = infos.iter().map(|f| f.path.clone()).collect();
        actual.sort();expected.sort();assert_eq!(actual,expected);
        println!("strict=true groups=1 grouped_files=3 byte_oracle_true_copies=2");
    }
}
