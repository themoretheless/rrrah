//! Controlled filesystem refusal probe; use only the supplied isolated test mounts.
use rrrah_dedup::{
    cache::{CacheError, CacheKey, FingerprintCache},
    pixels::Fingerprint,
};
use std::io::Write;

#[allow(clippy::too_many_lines)] // Keep controlled storage setup, refusal proof and retry in one probe.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let mode = args.get(1).ok_or("mode required")?;
    let folder = std::path::Path::new(args.get(2).ok_or("isolated test folder required")?);
    let path = folder.join("cache.bin");
    let key = CacheKey {
        content: [1; 32],
        recipe: [2; 32],
        frame_index: 0,
    };
    let original_fp = Fingerprint {
        variants: [[1; 4]; 8],
        mean_linear_rgb: [0.1; 3],
        luminance_stddev: 0.5,
    };
    let mut cache = FingerprintCache::default();
    cache.insert(key, original_fp.clone(), 1)?;
    if mode == "prepare" || mode == "full" {
        cache.save_atomic(&path, || false)?;
    }
    if mode == "prepare" {
        println!("{{\"prepared\":true}}");
        return Ok(());
    }
    if mode != "readonly" && mode != "full" && mode != "retry" {
        return Err("expected prepare, readonly, full or retry".into());
    }
    let old_bytes = std::fs::read(&path)?;
    assert_eq!(
        FingerprintCache::load_file(&path, 1, || false)?.get(&key),
        Some(&original_fp)
    );
    if mode == "retry" {
        let changed = Fingerprint {
            variants: [[2; 4]; 8],
            ..original_fp
        };
        cache.insert(key, changed.clone(), 1)?;
        cache.save_atomic(&path, || false)?;
        assert_eq!(
            FingerprintCache::load_file(&path, 1, || false)?.get(&key),
            Some(&changed)
        );
        assert_eq!(std::fs::read_dir(folder)?.count(), 1);
        println!("{{\"retry_after_readonly_remount\":true}}");
        return Ok(());
    }
    let filler = folder.join("filler.bin");
    if mode == "full" {
        let mut file = std::fs::File::create(&filler)?;
        let chunk = [0x5a; 4096];
        let mut exhausted = false;
        // Refuse to fill arbitrary storage: this probe expects a <=64 KiB tmpfs.
        for _ in 0..512 {
            match file.write_all(&chunk) {
                Ok(()) => {}
                Err(error) if error.raw_os_error() == Some(28) => {
                    exhausted = true;
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        assert!(exhausted, "isolated filesystem did not fill within 2 MiB guard");
    }
    let changed = Fingerprint {
        variants: [[2; 4]; 8],
        ..original_fp
    };
    cache.insert(key, changed.clone(), 1)?;
    let refusal = cache.save_atomic(&path, || false).unwrap_err();
    let CacheError::Io(error) = refusal else {
        panic!("expected pre-publication I/O failure: {refusal:?}");
    };
    let expected = if mode == "full" { 28 } else { 30 };
    let expected_kind = if mode == "full" {
        std::io::ErrorKind::StorageFull
    } else {
        std::io::ErrorKind::ReadOnlyFilesystem
    };
    assert_eq!(error.kind(), expected_kind);
    let raw_errno = error
        .raw_os_error()
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    if let Some(value) = error.raw_os_error() {
        assert_eq!(value, expected);
    }
    assert_eq!(std::fs::read(&path)?, old_bytes);
    assert_eq!(
        FingerprintCache::load_file(&path, 1, || false)?.get(&key),
        Some(&original_fp)
    );
    assert_eq!(
        std::fs::read_dir(folder)?.count(),
        if mode == "full" { 2 } else { 1 }
    );
    if mode == "full" {
        std::fs::remove_file(filler)?;
        cache.save_atomic(&path, || false)?;
        assert_eq!(
            FingerprintCache::load_file(&path, 1, || false)?.get(&key),
            Some(&changed)
        );
    }
    println!(
        "{{\"mode\":\"{mode}\",\"expected_errno\":{expected},\"raw_errno\":{raw_errno},\"previous_cache_preserved\":true,\"temporary_files_removed\":true,\"retry_checked\":{}}}",
        mode == "full"
    );
    Ok(())
}
