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
        let mode=std::env::var("DEDUP_BENCH_CONTENT").unwrap_or_else(|_|"equal".into());
        assert!(["equal","early","late","sizes","mixed"].contains(&mode.as_str()));
        assert!(mode=="equal"||size>0);
        let contents:Vec<Vec<u8>>=(0..n).map(|i|{
            let mut v=bytes.clone();
            match mode.as_str(){
                "early"=>v[0]^=(i+1)as u8,
                "late"=>v[size-1]^=(i+1)as u8,
                "sizes"=>v.extend(std::iter::repeat_n(71,i)),
                "mixed"=>v[0]^=(i%3)as u8,
                _=>{},
            }
            v
        }).collect();
        let oracle:Vec<Vec<bool>>=contents.iter().map(|a|contents.iter().map(|b|a==b).collect()).collect();
        let expected_matches:usize=(0..n).map(|i|(i+1..n).filter(|&j|oracle[i][j]).count()).sum();
        for (path,content) in paths.iter().zip(&contents){fs::write(path,content).unwrap();}
        let checkpoints=std::cell::Cell::new(0u64);
        let cancelled=||{checkpoints.set(checkpoints.get()+1);false};
        let callback:&dyn Fn()->bool=std::hint::black_box(&cancelled);
        let timer=std::time::Instant::now();let mut accepted=0usize;let mut matched=0usize;
        for _ in 0..iterations {
            let snapshots:Vec<_>=paths.iter().map(|p|ContentSnapshot::read(p,u64::MAX,callback).unwrap()).collect();
            for source in &snapshots {source.verify(callback).unwrap();}
            for i in 0..n {for j in i+1..n {
                let a=&snapshots[i];let b=&snapshots[j];
                let identical=if candidate {selective_verified_pair(a,b,callback).unwrap()}else{original_verified_pair(a,b,callback).unwrap()};
                assert_eq!(std::hint::black_box(identical),oracle[i][j]);matched+=usize::from(identical);
                a.verify(callback).unwrap();b.verify(callback).unwrap();accepted+=1;
            }}
            for source in &snapshots {source.verify(callback).unwrap();}
            // Preserve the callback-free publication revalidation pass.
            for source in &snapshots {source.verify(||false).unwrap();}
        }
        assert_eq!(accepted,iterations*n*(n-1)/2);
        assert_eq!(matched,iterations*expected_matches);
        println!("DEDUP_BENCH_NS={} ACCEPTED={accepted} MATCHED={matched}",timer.elapsed().as_nanos());
    }
}
