use rrrah_dedup::{cache::{CacheError, CacheKey, FingerprintCache}, pixels::Fingerprint};
use std::{cell::Cell, fs, path::PathBuf};

fn key(record: u8) -> CacheKey {
    CacheKey { content: [record; 32], recipe: [77; 32], frame_index: 0 }
}
fn cache(count: u8) -> FingerprintCache {
    let mut cache = FingerprintCache::default();
    for n in 0..count {
        cache.insert(key(n), Fingerprint {
            variants: [[u64::from(n); 4]; 8], mean_linear_rgb: [0.1, 0.2, 0.3],
            luminance_stddev: 0.4,
        }, 64).unwrap();
    }
    cache
}
fn main() {
    let directory = PathBuf::from(std::env::args_os().nth(1).unwrap());
    fs::create_dir(&directory).unwrap();
    let path = directory.join("owned-cache.bin");
    let old = cache(64);
    old.save_atomic(&path, || false).unwrap();
    let original = fs::read(&path).unwrap();
    let mut checkpoints = Vec::new();
    for count in [0, 1, 7] {
        let next = cache(count);
        let mut expected = Vec::new();
        next.write(&mut expected, || false).unwrap();
        assert!(expected.len() < original.len());
        let calls = Cell::new(0);
        next.save_atomic(&path, || { calls.set(calls.get() + 1); false }).unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        let total = calls.get();
        for stop in 1..=total {
            old.save_atomic(&path, || false).unwrap();
            let seen = Cell::new(0);
            let cancelled = next.save_atomic(&path, || {
                seen.set(seen.get() + 1);
                seen.get() == stop
            });
            assert!(matches!(cancelled, Err(CacheError::Cancelled)), "count={count} checkpoint={stop}: {cancelled:?}");
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
            next.save_atomic(&path, || false).unwrap();
            assert_eq!(fs::read(&path).unwrap(), expected);
            let loaded = FingerprintCache::load_file(&path, usize::from(count), || false).unwrap();
            for n in 0..64 { assert_eq!(loaded.get(&key(n)), next.get(&key(n))); }
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        }
        checkpoints.push((count, total, expected.len()));
    }
    fs::remove_file(path).unwrap();
    fs::remove_dir(directory).unwrap();
    println!("{{\"old_records\":64,\"old_bytes\":{},\"empty_cases\":{},\"one_record_cases\":{},\"seven_record_cases\":{},\"final_lengths\":[{},{},{}]}}", original.len(), checkpoints[0].1, checkpoints[1].1, checkpoints[2].1, checkpoints[0].2, checkpoints[1].2, checkpoints[2].2);
}
