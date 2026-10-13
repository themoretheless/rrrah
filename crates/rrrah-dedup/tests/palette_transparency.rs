#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::decode::decode_selected_frame;
#[test]
fn original_palette_alpha_tables_ignore_hidden_rgb_across_rgba_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    let bytes: [(&str, &[u8]); 4] = [
        (
            "a.png",
            include_bytes!("fixtures/palette-transparency/hidden-black.png"),
        ),
        (
            "b.png",
            include_bytes!("fixtures/palette-transparency/hidden-gradient.png"),
        ),
        (
            "c.png",
            include_bytes!("fixtures/palette-transparency/hidden-rgba.png"),
        ),
        (
            "d.png",
            include_bytes!("fixtures/palette-transparency/visible-center.png"),
        ),
    ];
    let paths: Vec<_> = bytes
        .iter()
        .map(|(name, data)| {
            let p = dir.path().join(name);
            std::fs::write(&p, data).unwrap();
            p
        })
        .collect();
    {
        let load = |i: usize| {
            let mut r = DecodeRequest::new(&paths[i]);
            r.assume_untagged_linear_srgb = true;
            decode_selected_frame(&r, 81, &budget, || false).unwrap()
        };
        let a = load(0);
        assert!(a.same_selected_frame(&load(1), || false).unwrap());
        assert!(a.same_selected_frame(&load(2), || false).unwrap());
        assert!(!a.same_selected_frame(&load(3), || false).unwrap());
    }
    assert_eq!(budget.used(), 0);
}
#[test]
fn original_uniform_png_hash_collision_does_not_prove_pixel_equality() {
    let dir = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(1024 * 1024);
    {
        let load = |name: &str, data: &[u8]| {
            let p = dir.path().join(name);
            std::fs::write(&p, data).unwrap();
            let mut r = DecodeRequest::new(p);
            r.assume_untagged_linear_srgb = true;
            decode_selected_frame(&r, 81, &budget, || false).unwrap()
        };
        let a = load("a.png", include_bytes!("fixtures/palette-transparency/black.png"));
        let b = load("b.png", include_bytes!("fixtures/palette-transparency/white.png"));
        assert!(!a.same_selected_frame(&b, || false).unwrap());
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_palette_collection_matches_exhaustive_without_uniform_false_edges() {
    use rrrah_dedup::{
        decode::FingerprintPolicy,
        pixel_index::scan_indexed_pixels,
        scan::{PixelSearchPolicy, scan_equal_pixels},
    };
    let dir = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let data: [&[u8]; 6] = [
        include_bytes!("fixtures/palette-transparency/hidden-black.png"),
        include_bytes!("fixtures/palette-transparency/hidden-gradient.png"),
        include_bytes!("fixtures/palette-transparency/hidden-rgba.png"),
        include_bytes!("fixtures/palette-transparency/visible-center.png"),
        include_bytes!("fixtures/palette-transparency/black.png"),
        include_bytes!("fixtures/palette-transparency/white.png"),
    ];
    let mut files: Vec<_> = data
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let p = dir.path().join(format!("{i}.png"));
            std::fs::write(&p, bytes).unwrap();
            let mut r = DecodeRequest::new(p);
            r.assume_untagged_linear_srgb = true;
            (i as u64 + 1, r)
        })
        .collect();
    let policy = PixelSearchPolicy {
        decode: FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: 4096,
            max_pixels: 81,
            max_frames: 1,
            max_cache_entries: 0,
        },
        max_files: 6,
        max_pairs: 15,
    };
    for _ in 0..2 {
        let exhaustive = scan_equal_pixels(files.clone(), policy, &budget, || false).unwrap();
        assert_eq!(exhaustive.pixels.equal, [(1, 2), (1, 3), (2, 3)]);
        assert_eq!(exhaustive.pixels.different.len(), 12);
        assert!(exhaustive.pixels.issues.is_empty() && exhaustive.source_issues.is_empty());
        assert_eq!(budget.used(), 0);
        let indexed = scan_indexed_pixels(files.clone(), policy, &budget, || false).unwrap();
        assert_eq!(indexed.pixels.equal, exhaustive.pixels.equal);
        assert_eq!(indexed.indexed_decodes, 6);
        assert_eq!(indexed.candidate_pairs, 3);
        assert!(
            indexed.pixels.different.is_empty()
                && indexed.pixels.issues.is_empty()
                && indexed.issues.is_empty()
                && indexed.source_issues.is_empty()
        );
        assert_eq!(budget.used(), 0);
        files.reverse();
    }
}
