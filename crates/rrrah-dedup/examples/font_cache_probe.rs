//! Two-process probe: compare persisted evidence with a fresh decode under current fonts.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("expected SVG path and persisted cache path".into());
    }
    let request = DecodeRequest::new(&args[0]);
    let cache_path = std::path::Path::new(&args[1]);
    let policy = FingerprintPolicy {
        recipe: [43; 32],
        max_file_bytes: 1_048_576,
        max_pixels: 1_048_576,
        max_frames: 1,
        max_cache_entries: 4,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut cache = if cache_path.exists() {
        FingerprintCache::load_file(cache_path, 4, || false)?
    } else {
        FingerprintCache::default()
    };
    let reused = fingerprint_file(&request, policy, &budget, &mut cache, || false)?;
    println!("cached_result_hit={}", reused.cache_hit);
    let fresh = fingerprint_file(
        &request,
        policy,
        &budget,
        &mut FingerprintCache::default(),
        || false,
    )?;
    println!(
        "cache_hit={} equal_to_fresh={} reused={:?} fresh={:?}",
        reused.cache_hit,
        reused.fingerprint == fresh.fingerprint,
        reused.fingerprint,
        fresh.fingerprint
    );
    cache.save_atomic(cache_path, || false)?;
    assert_eq!(budget.used(), 0);
    if reused.fingerprint != fresh.fingerprint {
        return Err("persisted cache differs from fresh decode".into());
    }
    Ok(())
}
