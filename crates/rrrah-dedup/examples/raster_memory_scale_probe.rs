//! Authored pixel-dimension probe; fixture generation is outside decode timing.
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    time::Instant,
};

// Keep the timed probe and its refusal/retry/equality checks in one sequence.
#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let png_folder = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let directory = tempfile::tempdir()?;
    for side in [1024u32, 2048, 4096] {
        let bytes = 54 + side * side * 3;
        let path = directory.path().join(format!("{side}.bmp"));
        let mut header = [0u8; 54];
        header[..2].copy_from_slice(b"BM");
        for (offset, value) in [
            (2, bytes),
            (10, 54),
            (14, 40),
            (18, side),
            (22, side),
            (34, bytes - 54),
        ] {
            header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        header[26..28].copy_from_slice(&1u16.to_le_bytes());
        header[28..30].copy_from_slice(&24u16.to_le_bytes());
        let mut file = File::create(&path)?;
        file.write_all(&header)?;
        let mut row = vec![0u8; usize::try_from(side)? * 3];
        for y in 0..side {
            for x in 0..side {
                let at = usize::try_from(x)? * 3;
                row[at..at + 3].copy_from_slice(&[(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8]);
            }
            file.write_all(&row)?;
        }
        drop((file, row));
        let request = DecodeRequest::new(path);
        let policy = FingerprintPolicy {
            recipe: [17; 32],
            max_file_bytes: u64::from(bytes),
            max_pixels: u64::from(side) * u64::from(side),
            max_frames: 1,
            max_cache_entries: 1,
        };
        let mut cache = FingerprintCache::default();
        for available in [0, 1] {
            let budget = MemoryBudget::new(available);
            assert!(fingerprint_file(&request, policy, &budget, &mut cache, || false).is_err());
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(1024 * 1024 * 1024);
        let start = Instant::now();
        let decoded = fingerprint_file(&request, policy, &budget, &mut cache, || false)?;
        let decode_ns = start.elapsed().as_nanos();
        assert!(!decoded.cache_hit, "refusals must not admit a cache entry");
        assert_eq!(budget.used(), 0);
        let no_output_budget = MemoryBudget::new(0);
        let hit = fingerprint_file(&request, policy, &no_output_budget, &mut cache, || false)?;
        assert!(hit.cache_hit);
        assert_eq!(hit.fingerprint, decoded.fingerprint);
        assert_eq!(no_output_budget.used(), 0);
        println!(
            "{{\"side\":{side},\"pixels\":{},\"file_bytes\":{bytes},\"decode_ns\":{decode_ns},\"managed_used_after\":0}}",
            u64::from(side) * u64::from(side)
        );
        if let Some(folder) = &png_folder {
            for format in ["png", "tiff"] {
                let mut png_request = DecodeRequest::new(folder.join(format!("{side}.{format}")));
                if format == "tiff" {
                    png_request.assume_untagged_srgb = true;
                }
                let png_policy = FingerprintPolicy {
                    max_file_bytes: std::fs::metadata(&png_request.path)?.len(),
                    ..policy
                };
                let mut png_cache = FingerprintCache::default();
                for available in [0, 1] {
                    let refused_budget = MemoryBudget::new(available);
                    assert!(
                        fingerprint_file(&png_request, png_policy, &refused_budget, &mut png_cache, || {
                            false
                        })
                        .is_err()
                    );
                    assert_eq!(refused_budget.used(), 0);
                }
                let start = Instant::now();
                let png = fingerprint_file(&png_request, png_policy, &budget, &mut png_cache, || false)?;
                let decode_ns = start.elapsed().as_nanos();
                assert!(!png.cache_hit);
                assert_eq!(png.fingerprint, decoded.fingerprint, "BMP/PNG {side}");
                assert_eq!(budget.used(), 0);
                let hit = fingerprint_file(
                    &png_request,
                    png_policy,
                    &no_output_budget,
                    &mut png_cache,
                    || false,
                )?;
                assert!(hit.cache_hit);
                assert_eq!(hit.fingerprint, png.fingerprint);
                assert_eq!(no_output_budget.used(), 0);
                let left =
                    rrrah_dedup::decode::decode_selected_frame(&request, policy.max_pixels, &budget, || {
                        false
                    })?;
                let right = rrrah_dedup::decode::decode_selected_frame(
                    &png_request,
                    policy.max_pixels,
                    &budget,
                    || false,
                )?;
                assert!(left.same_selected_frame(&right, || false)?);
                drop(left);
                let mut changed_file = File::options().read(true).write(true).open(&request.path)?;
                let offset = 54 + policy.max_pixels / 2 * 3;
                changed_file.seek(SeekFrom::Start(offset))?;
                let mut byte = [0];
                changed_file.read_exact(&mut byte)?;
                let original_byte = byte[0];
                byte[0] ^= 1;
                changed_file.seek(SeekFrom::Start(offset))?;
                changed_file.write_all(&byte)?;
                drop(changed_file);
                let changed =
                    rrrah_dedup::decode::decode_selected_frame(&request, policy.max_pixels, &budget, || {
                        false
                    })?;
                assert!(!changed.same_selected_frame(&right, || false)?);
                drop((changed, right));
                assert_eq!(budget.used(), 0);
                let mut restore = File::options().write(true).open(&request.path)?;
                restore.seek(SeekFrom::Start(offset))?;
                restore.write_all(&[original_byte])?;
                drop(restore);
                println!(
                    "{{\"format\":\"{format}\",\"side\":{side},\"exact_pixels_equal\":true,\"one_byte_change_rejected\":true}}"
                );
                println!(
                    "{{\"format\":\"{format}\",\"side\":{side},\"decode_ns\":{decode_ns},\"cross_format_equal\":true}}"
                );
            }
        }
    }
    #[cfg(target_os = "linux")]
    for line in std::fs::read_to_string("/proc/self/status")?.lines() {
        if line.starts_with("VmHWM:") || line.starts_with("VmRSS:") {
            eprintln!("process_memory {line}");
        }
    }
    Ok(())
}
