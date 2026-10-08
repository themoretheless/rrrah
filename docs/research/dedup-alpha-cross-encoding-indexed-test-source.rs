#![cfg(feature = "decode")]
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{decode::decode_selected_frame, raster::NormalizedRaster, scan::confirm_pixels};
use std::{path::PathBuf, sync::Arc};

fn request(path: PathBuf) -> DecodeRequest {
    let mut request = DecodeRequest::new(path);
    request.assume_untagged_srgb = true;
    request
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raster-equality")
}

#[test]
fn independent_lossless_encodings_preserve_pixels_alpha_and_one_code_negatives() {
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    for (kind, width, height, extensions) in [
        (
            "opaque",
            19,
            13,
            &["png", "tiff", "bmp", "ppm", "tga", "webp", "qoi"][..],
        ),
        ("alpha", 17, 11, &["png", "tiff", "tga", "webp", "qoi"][..]),
    ] {
        let pixels = std::fs::read(root().join(format!("{kind}.rgba"))).unwrap();
        let source = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba8(Arc::new(pixels).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap();
        let expected = NormalizedRaster::new(&source, 1000, &budget, || false).unwrap();
        let expected_digest = expected.view(|| false).unwrap().pixel_digest(|| false).unwrap();
        for extension in extensions {
            for variant in if kind == "alpha" {
                &['b', 'c', 'h'][..]
            } else {
                &['b', 'c'][..]
            } {
                let variant = match variant {
                    'b' => "base",
                    'c' => "changed",
                    _ => "hidden",
                };
                let path = root().join(format!("{kind}-{variant}.{extension}"));
                let decoded = decode_selected_frame(&request(path.clone()), 1000, &budget, || false)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                let equal = expected.same_selected_frame(&decoded, || false).unwrap();
                assert_eq!(equal, variant != "changed", "{}", path.display());
                let digest = decoded.view(|| false).unwrap().pixel_digest(|| false).unwrap();
                assert_eq!(digest == expected_digest, equal, "{}", path.display());
            }
        }
    }
    assert_eq!(budget.used(), 0);
}

#[test]
fn cross_encoding_batch_confirms_every_pair_and_rejects_single_pixel_edits() {
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let extensions = ["png", "tiff", "bmp", "ppm", "tga", "webp", "qoi"];
    let mut files = extensions
        .iter()
        .enumerate()
        .map(|(id, ext)| (id as u64, request(root().join(format!("opaque-base.{ext}")))))
        .collect::<Vec<_>>();
    files.push((7, request(root().join("opaque-changed.png"))));
    let pairs = (0..8)
        .flat_map(|a| (a + 1..8).map(move |b| (a, b)))
        .collect::<Vec<_>>();
    let result = confirm_pixels(
        files,
        pairs,
        rrrah_dedup::scan::VisualPolicy {
            fingerprint: rrrah_dedup::decode::FingerprintPolicy {
                recipe: [1; 32],
                max_file_bytes: 1024 * 1024,
                max_pixels: 1000,
                max_frames: 10,
                max_cache_entries: 10,
            },
            max_files: 8,
            max_pairs: 28,
            radius: 0,
            allow_transforms: false,
            require_information: true,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert!(result.issues.is_empty());
    assert_eq!(result.equal.len(), 21);
    assert_eq!(result.different.len(), 7);
    assert!(result.equal.iter().all(|&(a, b)| a < 7 && b < 7));
    assert!(result.different.iter().all(|&(_, b)| b == 7));
    assert_eq!(budget.used(), 0);
}

#[test]
fn untagged_policy_is_explicit_cached_and_does_not_override_declared_linear_color() {
    use rrrah_dedup::{
        cache::FingerprintCache,
        decode::{FingerprintPolicy, fingerprint_file},
    };
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    for ext in ["tiff", "tga"] {
        let path = root().join(format!("opaque-base.{ext}"));
        assert!(decode_selected_frame(&DecodeRequest::new(&path), 1000, &budget, || false).is_err());
        drop(decode_selected_frame(&request(path), 1000, &budget, || false).unwrap());
        assert_eq!(budget.used(), 0);
    }
    // Standard PPM defines BT.709; it must not be classified as unknown color.
    let ppm_path = root().join("opaque-base.ppm");
    let ppm = rrrah_decode::decode_raster(&DecodeRequest::new(&ppm_path)).unwrap();
    assert_eq!(ppm.color_space(), &RasterColorSpace::Bt709);
    drop(ppm);
    drop(decode_selected_frame(&DecodeRequest::new(&ppm_path), 1000, &budget, || false).unwrap());
    assert_eq!(budget.used(), 0);
    let srgb =
        decode_selected_frame(&request(root().join("opaque-base.qoi")), 1000, &budget, || false).unwrap();
    let explicit_linear = DecodeRequest::new(root().join("opaque-linear.qoi"));
    let linear = decode_selected_frame(&explicit_linear, 1000, &budget, || false).unwrap();
    let assumed =
        decode_selected_frame(&request(explicit_linear.path.clone()), 1000, &budget, || false).unwrap();
    assert!(!srgb.same_selected_frame(&linear, || false).unwrap());
    assert!(linear.same_selected_frame(&assumed, || false).unwrap());
    drop((srgb, linear, assumed));
    let tagged_path = root().join("opaque-tagged.png");
    let decoded = rrrah_decode::decode_raster(&request(tagged_path.clone())).unwrap();
    assert!(matches!(decoded.color_space(), RasterColorSpace::Icc(_)));
    drop(decoded);
    let strict_tagged =
        decode_selected_frame(&DecodeRequest::new(&tagged_path), 1000, &budget, || false).unwrap();
    let assumed_tagged = decode_selected_frame(&request(tagged_path), 1000, &budget, || false).unwrap();
    assert!(
        strict_tagged
            .same_selected_frame(&assumed_tagged, || false)
            .unwrap()
    );
    drop((strict_tagged, assumed_tagged));

    let policy = FingerprintPolicy {
        recipe: [2; 32],
        max_file_bytes: 1024 * 1024,
        max_pixels: 1000,
        max_frames: 10,
        max_cache_entries: 10,
    };
    let path = root().join("opaque-base.tiff");
    let mut cache = FingerprintCache::default();
    assert!(
        !fingerprint_file(&request(path.clone()), policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        fingerprint_file(&request(path.clone()), policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    // Strict policy must execute and reject unknown source color, never use the assumed result.
    assert!(fingerprint_file(&DecodeRequest::new(path), policy, &budget, &mut cache, || false).is_err());
    let png = root().join("opaque-base.png");
    assert!(
        !fingerprint_file(&DecodeRequest::new(&png), policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert!(
        !fingerprint_file(&request(png), policy, &budget, &mut cache, || false)
            .unwrap()
            .cache_hit
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn associated_tiff_alpha_matches_independent_straight_png() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/associated-alpha");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let png = decode_selected_frame(&request(folder.join("straight.png")), 64, &budget, || false).unwrap();
    let mut names = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tiff"))
        })
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names.len(), 15);
    for name in names {
        let tiff = decode_selected_frame(&request(folder.join(&name)), 64, &budget, || false).unwrap();
        assert!(png.same_selected_frame(&tiff, || false).unwrap(), "{name}");
    }
    drop(png);
    assert_eq!(budget.used(), 0);
}

#[test]
fn malformed_associated_alpha_refuses_releases_memory_and_retries() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/associated-alpha");
    let original = std::fs::read(folder.join("associated.tiff")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("retry.tiff");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    // Authored classic little-endian directory: last tag is ExtraSamples.
    assert_eq!(&original[142..144], &338u16.to_le_bytes());
    for (offset, bytes) in [
        (150, vec![3, 0]),       // Unknown alpha interpretation.
        (144, vec![4, 0]),       // ExtraSamples requires SHORT.
        (146, vec![2, 0, 0, 0]), // Multiple extra channels are unqualified.
        (166, vec![1]),          // Associated nonzero RGB at alpha zero.
    ] {
        let mut damaged = original.clone();
        damaged[offset..offset + bytes.len()].copy_from_slice(&bytes);
        std::fs::write(&path, damaged).unwrap();
        assert!(
            decode_selected_frame(&request(path.clone()), 64, &budget, || false).is_err(),
            "offset {offset}"
        );
        assert_eq!(budget.used(), 0, "offset {offset}");
        std::fs::write(&path, &original).unwrap();
        let recovered = decode_selected_frame(&request(path.clone()), 64, &budget, || false).unwrap();
        let expected =
            decode_selected_frame(&request(folder.join("straight.png")), 64, &budget, || false).unwrap();
        assert!(recovered.same_selected_frame(&expected, || false).unwrap());
        drop((recovered, expected));
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn indexed_cross_encoding_collection_preserves_all_equal_pairs_and_one_pixel_negative() {
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let extensions = ["png", "tiff", "bmp", "ppm", "tga", "webp", "qoi"];
    let mut files = extensions.iter().enumerate().map(|(id, ext)| {
        (id as u64, request(root().join(format!("opaque-base.{ext}"))))
    }).collect::<Vec<_>>();
    files.push((7, request(root().join("opaque-changed.png"))));
    let policy = rrrah_dedup::scan::PixelSearchPolicy {
        decode: rrrah_dedup::decode::FingerprintPolicy {
            recipe: [1; 32], max_file_bytes: 1024 * 1024, max_pixels: 1000,
            max_frames: 10, max_cache_entries: 0,
        }, max_files: 8, max_pairs: 28,
    };
    let expected = (0..7).flat_map(|a| (a + 1..7).map(move |b| (a, b))).collect::<Vec<_>>();
    for reverse in [false, true] {
        if reverse { files.reverse(); }
        let result = rrrah_dedup::pixel_index::scan_indexed_pixels(
            files.clone(), policy, &budget, || false,
        ).unwrap();
        assert_eq!(result.analysed, (0..8).collect::<Vec<_>>());
        assert!(result.issues.is_empty() && result.source_issues.is_empty());
        assert_eq!(result.indexed_decodes, 8);
        assert_eq!(result.candidate_pairs, 21);
        assert_eq!(result.pixels.equal, expected);
        assert!(result.pixels.different.is_empty() && result.pixels.issues.is_empty());
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn indexed_alpha_encodings_ignore_hidden_rgb_and_separate_visible_edits() {
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let extensions = ["png", "tiff", "tga", "webp", "qoi"];
    let files = ["base", "hidden", "changed"].iter().enumerate().flat_map(|(group, variant)| {
        extensions.iter().enumerate().map(move |(format, ext)| {
            ((group * 5 + format) as u64, request(root().join(format!("alpha-{variant}.{ext}"))))
        })
    }).collect::<Vec<_>>();
    let expected = (0..15).flat_map(|a| (a + 1..15).filter_map(move |b| {
        ((a < 10) == (b < 10)).then_some((a, b))
    })).collect::<Vec<_>>();
    assert_eq!(expected.len(), 55);
    let policy = rrrah_dedup::scan::PixelSearchPolicy {
        decode: rrrah_dedup::decode::FingerprintPolicy {
            recipe: [1; 32], max_file_bytes: 1024 * 1024, max_pixels: 1000,
            max_frames: 10, max_cache_entries: 0,
        }, max_files: 15, max_pairs: 105,
    };
    for order in [files.clone(), files.into_iter().rev().collect()] {
        let result = rrrah_dedup::pixel_index::scan_indexed_pixels(order, policy, &budget, || false).unwrap();
        assert_eq!(result.analysed, (0..15).collect::<Vec<_>>());
        assert!(result.issues.is_empty() && result.source_issues.is_empty());
        assert_eq!(result.indexed_decodes, 15);
        assert_eq!(result.candidate_pairs, 55);
        assert_eq!(result.pixels.equal, expected);
        assert!(result.pixels.different.is_empty() && result.pixels.issues.is_empty());
        assert_eq!(budget.used(), 0);
    }
}
