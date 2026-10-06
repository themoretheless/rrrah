//! Warm-cache source verification cost; fixture generation and first decode are untimed.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
    exact::ContentSnapshot,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    for mib in [1usize, 8, 64] {
        let length = mib * 1024 * 1024;
        let mut bytes = vec![0x5a; length];
        bytes[..54].fill(0);
        bytes[..2].copy_from_slice(b"BM");
        for (offset, value) in [(2, 58u32), (10, 54), (14, 40), (18, 1), (22, 1), (34, 4)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
        bytes[54..58].copy_from_slice(&[0, 0, 255, 0]);
        let path = folder.path().join(format!("{mib}.bmp"));
        std::fs::write(&path, bytes)?;
        let policy = FingerprintPolicy {
            recipe: [1; 32],
            max_file_bytes: u64::try_from(length)?,
            max_pixels: 1,
            max_frames: 10,
            max_cache_entries: 10,
        };
        let budget = MemoryBudget::new(256 * 1024 * 1024);
        let request = DecodeRequest::new(&path);
        let mut cache = FingerprintCache::default();
        let original = fingerprint_file(&request, policy, &budget, &mut cache, || false)?;
        assert!(!original.cache_hit);
        for run in 0..6 {
            let start = Instant::now();
            let source = ContentSnapshot::read(&path, policy.max_file_bytes, || false)?;
            let single_read_ns = start.elapsed().as_nanos();
            let start = Instant::now();
            let result = fingerprint_file(&request, policy, &budget, &mut cache, || false)?;
            let cache_hit_ns = start.elapsed().as_nanos();
            assert!(result.cache_hit);
            assert_eq!(result.fingerprint, original.fingerprint);
            assert_eq!(source.byte_count(), u64::try_from(length)?);
            assert_eq!(budget.used(), 0);
            if run > 0 {
                println!(
                    "{{\"bytes\":{length},\"run\":{run},\"single_read_ns\":{single_read_ns},\"cache_hit_ns\":{cache_hit_ns}}}"
                );
            }
        }
    }
    Ok(())
}
