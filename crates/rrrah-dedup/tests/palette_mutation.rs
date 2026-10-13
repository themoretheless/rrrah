#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    cache::FingerprintCache,
    decode::{FingerprintPolicy, fingerprint_file},
    pixel_index::scan_indexed_pixels,
    scan::PixelSearchPolicy,
};
fn expose_palette_entry(mut bytes: Vec<u8>, entry: usize) -> Vec<u8> {
    let mut offset = 8;
    loop {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        if &bytes[offset + 4..offset + 8] == b"tRNS" {
            assert_eq!(length, 256);
            bytes[offset + 8 + entry] = 255;
            let mut crc = u32::MAX;
            for &byte in &bytes[offset + 4..offset + 8 + length] {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
                }
            }
            bytes[offset + 8 + length..offset + 12 + length].copy_from_slice(&(!crc).to_be_bytes());
            return bytes;
        }
        offset += length + 12;
        assert!(offset + 12 <= bytes.len());
    }
}
#[test]
fn same_length_png_alpha_table_rewrite_invalidates_cache_and_collection_edges() {
    let dir = tempfile::tempdir().unwrap();
    let original = include_bytes!("fixtures/palette-transparency/hidden-gradient.png");
    let mut files = Vec::new();
    for id in 1..=3 {
        let p = dir.path().join(format!("{id}.png"));
        std::fs::write(&p, original).unwrap();
        let mut r = DecodeRequest::new(p);
        r.assume_untagged_linear_srgb = true;
        files.push((id, r));
    }
    let policy = FingerprintPolicy {
        recipe: [19; 32],
        max_file_bytes: 4096,
        max_pixels: 81,
        max_frames: 1,
        max_cache_entries: 8,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut cache = FingerprintCache::default();
    let first = fingerprint_file(&files[1].1, policy, &budget, &mut cache, || false).unwrap();
    assert!(!first.cache_hit);
    let cached = fingerprint_file(&files[1].1, policy, &budget, &mut cache, || false).unwrap();
    assert!(cached.cache_hit);
    assert_eq!(first.fingerprint, cached.fingerprint);
    let search = PixelSearchPolicy {
        decode: policy,
        max_files: 3,
        max_pairs: 3,
    };
    let before = scan_indexed_pixels(files.clone(), search, &budget, || false).unwrap();
    assert_eq!(before.pixels.equal, [(1, 2), (1, 3), (2, 3)]);
    let changed = expose_palette_entry(original.to_vec(), 80);
    assert_eq!(changed.len(), original.len());
    assert_ne!(changed.as_slice(), original);
    std::fs::write(&files[1].1.path, &changed).unwrap();
    let fresh = fingerprint_file(&files[1].1, policy, &budget, &mut cache, || false).unwrap();
    assert!(!fresh.cache_hit);
    assert_ne!(first.fingerprint, fresh.fingerprint);
    let repeated = fingerprint_file(&files[1].1, policy, &budget, &mut cache, || false).unwrap();
    assert!(repeated.cache_hit);
    assert_eq!(fresh.fingerprint, repeated.fingerprint);
    files.reverse();
    let after = scan_indexed_pixels(files, search, &budget, || false).unwrap();
    assert_eq!(after.pixels.equal, [(1, 3)]);
    assert_eq!(after.candidate_pairs, 1);
    assert!(after.issues.is_empty() && after.source_issues.is_empty() && after.pixels.issues.is_empty());
    assert_eq!(budget.used(), 0);
}
