#[cfg(test)]
mod actual_collection_benchmark {
    use super::*;
    #[test]
    #[ignore = "manual complete collection engine benchmark with synthetic provider"]
    fn complete_guarded_engine() {
        let n: usize = std::env::var("DEDUP_BENCH_FILES").unwrap().parse().unwrap();
        let size: usize = std::env::var("DEDUP_BENCH_BYTES").unwrap().parse().unwrap();
        let iterations: usize = std::env::var("DEDUP_BENCH_ITERATIONS").unwrap().parse().unwrap();
        let mode = std::env::var("DEDUP_BENCH_CONTENT").unwrap();
        assert!((2..=24).contains(&n) && size <= 2*1024*1024 && (1..=1000).contains(&iterations));
        assert!(["equal", "early", "late", "sizes", "mixed"].contains(&mode.as_str()));
        assert!(mode == "equal" || size >= 128);
        let dir = tempfile::tempdir().unwrap();
        let contents: Vec<Vec<u8>> = (0..n).map(|i| {
            let mut bytes: Vec<u8> = (0..size).map(|j| (j.wrapping_mul(37)%251) as u8).collect();
            // Public synthetic BMP-like header; dependency observation only,
            // no assertion of real pixel decoding or provider matching quality.
            if size >= 128 {
                bytes[..54].fill(0); bytes[..2].copy_from_slice(b"BM");
                bytes[2..6].copy_from_slice(&(size as u32).to_le_bytes());
                bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
                bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
                bytes[18..22].copy_from_slice(&1i32.to_le_bytes());
                bytes[22..26].copy_from_slice(&1i32.to_le_bytes());
                bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
                bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
            }
            match mode.as_str() {
                "early" => bytes[54] ^= (i+1) as u8,
                "late" => bytes[size-1] ^= (i+1) as u8,
                "sizes" => bytes.extend(std::iter::repeat_n(71,i)),
                "mixed" => bytes[54] ^= (i%3) as u8,
                _ => {},
            }
            bytes
        }).collect();
        let oracle: Vec<Vec<bool>> = contents.iter().map(|a|contents.iter().map(|b|a==b).collect()).collect();
        let files: Vec<_> = contents.iter().enumerate().map(|(i,b)| {
            let p=dir.path().join(format!("{i}.bmp")); std::fs::write(&p,b).unwrap();
            (i as u64,DecodeRequest::new(p))
        }).collect();
        let weights=dir.path().join("weights");std::fs::write(&weights,b"synthetic provider").unwrap();
        let pair_count=n*(n-1)/2;
        let expected_visual: usize=(0..n).map(|i|(i+1..n).filter(|&j|!oracle[i][j]).count()).sum();
        let budget=MemoryBudget::new(64*1024*1024);
        let calls=Cell::new(0u64);let callback=||{calls.set(calls.get()+1);false};
        let cancel:&dyn Fn()->bool=std::hint::black_box(&callback);
        let timer=std::time::Instant::now();let mut accepted=0usize;let mut visual_calls=0usize;
        for _ in 0..iterations {
            let report=scan_guarded_exact_collection(files.clone(),&weights,
                |_|Ok(CollectionSettings{max_files:n,max_pairs:pair_count,max_model_bytes:1024,max_file_bytes:u64::MAX}),
                &budget,|a,b,_|->Result<bool,std::io::Error>{
                    let index=|r:&DecodeRequest|r.path.file_stem().unwrap().to_str().unwrap().parse::<usize>().unwrap();
                    visual_calls+=1;Ok(oracle[index(a)][index(b)])
                },||true,cancel).unwrap();
            assert_eq!(report.pairs().len(),pair_count);
            for pair in report.pairs(){assert_eq!(pair.evidence,oracle[pair.left as usize][pair.right as usize]);accepted+=1;}
            drop(report);assert_eq!(budget.used(),0);
        }
        assert_eq!(accepted,iterations*pair_count);assert_eq!(visual_calls,iterations*expected_visual);
        println!("DEDUP_ENGINE_NS={} ACCEPTED={accepted} VISUAL_CALLS={visual_calls} CHECKPOINTS={}",timer.elapsed().as_nanos(),calls.get());
    }
}
