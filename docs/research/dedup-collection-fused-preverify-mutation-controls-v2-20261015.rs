#[cfg(test)]
mod fused_preverify_mutation_controls {
    use super::*;
    #[test]
    fn equal_rewrites_with_current_metadata_still_reject_original_digest() {
        // Explicit observation adapter isolates digest validation from OS stamps.
        let dir=tempfile::tempdir().unwrap();
        let a=dir.path().join("a");let b=dir.path().join("b");
        fs::write(&a,b"AAAA").unwrap();fs::write(&b,b"AAAA").unwrap();
        let digest=ContentSnapshot::read(&a,4,||false).unwrap().digest;
        fs::write(&a,b"BBBB").unwrap();fs::write(&b,b"BBBB").unwrap();
        let mut left=ContentSnapshot::read(&a,4,||false).unwrap();
        let mut right=ContentSnapshot::read(&b,4,||false).unwrap();
        left.digest=digest;right.digest=digest;
        assert!(matches!(compare_verified_observed_candidate(&left,&right,&||false),Err((true,SnapshotError::Changed))));
    }
    #[test]
    fn same_length_restored_mtime_mutation_at_every_checkpoint_is_rejected() {
        let dir=tempfile::tempdir().unwrap();let a=dir.path().join("a");let b=dir.path().join("b");
        let bytes=vec![17;BLOCK+3];
        fs::write(&a,&bytes).unwrap();fs::write(&b,&bytes).unwrap();
        let left=ContentSnapshot::read(&a,u64::MAX,||false).unwrap();let right=ContentSnapshot::read(&b,u64::MAX,||false).unwrap();
        let calls=std::cell::Cell::new(0);
        assert!(compare_verified_observed_candidate(&left,&right,&||{calls.set(calls.get()+1);false}).unwrap());
        for side in [true,false] {for point in 1..=calls.get(){
            fs::write(&a,&bytes).unwrap();fs::write(&b,&bytes).unwrap();
            let left=ContentSnapshot::read(&a,u64::MAX,||false).unwrap();let right=ContentSnapshot::read(&b,u64::MAX,||false).unwrap();
            let path=if side{&a}else{&b};let modified=fs::metadata(path).unwrap().modified().unwrap();let count=std::cell::Cell::new(0);
            let result=compare_verified_observed_candidate(&left,&right,&||{
                count.set(count.get()+1);
                if count.get()==point {
                    fs::write(path,vec![29;bytes.len()]).unwrap();
                    OpenOptions::new().write(true).open(path).unwrap().set_times(fs::FileTimes::new().set_modified(modified)).unwrap();
                    assert_eq!(fs::metadata(path).unwrap().modified().unwrap(),modified);
                }
                false
            });
            assert!(matches!(result,Err((_,SnapshotError::Changed))),"side={side} checkpoint={point}: {result:?}");
        }}
    }
}
