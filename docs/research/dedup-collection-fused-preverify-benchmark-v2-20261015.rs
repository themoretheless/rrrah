#[cfg(test)]
mod fused_preverify_benchmark {
    use super::*;
    #[test]
    #[ignore = "manual same-binary alternating benchmark; not a correctness gate"]
    fn full_exact_guard_loop() {
        let n:usize=std::env::var("DEDUP_BENCH_FILES").unwrap().parse().unwrap();
        let size:usize=std::env::var("DEDUP_BENCH_BYTES").unwrap().parse().unwrap();
        let iterations:usize=std::env::var("DEDUP_BENCH_ITERATIONS").unwrap().parse().unwrap();
        let candidate=std::env::var("DEDUP_BENCH_VARIANT").unwrap()=="candidate";
        assert!((2..=24).contains(&n)&&size<=2*1024*1024&&iterations<=1000);
        let dir=tempfile::tempdir().unwrap();
        let paths:Vec<_>=(0..n).map(|i|dir.path().join(i.to_string())).collect();
        let bytes:Vec<u8>=(0..size).map(|i|(i.wrapping_mul(37)%251)as u8).collect();
        for path in &paths {fs::write(path,&bytes).unwrap();}
        let timer=std::time::Instant::now();let mut accepted=0usize;
        for _ in 0..iterations {
            let snapshots:Vec<_>=paths.iter().map(|p|ContentSnapshot::read(p,u64::MAX,||false).unwrap()).collect();
            for source in &snapshots {source.verify(||false).unwrap();}
            for i in 0..n {for j in i+1..n {
                let a=&snapshots[i];let b=&snapshots[j];
                let identical=if candidate {
                    compare_verified_observed_candidate(a,b,&||false).unwrap()
                }else{
                    a.verify(||false).unwrap();b.verify(||false).unwrap();
                    compare_observed_tagged(a,b,&||false).unwrap()
                };
                assert!(std::hint::black_box(identical));
                a.verify(||false).unwrap();b.verify(||false).unwrap();accepted+=1;
            }}
            for source in &snapshots {source.verify(||false).unwrap();}
            // Preserve the callback-free publication revalidation pass.
            for source in &snapshots {source.verify(||false).unwrap();}
        }
        assert_eq!(accepted,iterations*n*(n-1)/2);
        println!("DEDUP_BENCH_NS={} ACCEPTED={accepted}",timer.elapsed().as_nanos());
    }
}
