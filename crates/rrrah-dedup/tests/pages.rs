#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    pages::{PageKind, decode_pages},
};
#[test]
fn all_dcx_pages_and_changed_second_page_are_compared() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let path = root.join("two-pages.dcx");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let first = decode_pages(&DecodeRequest::new(&path), PageKind::Dcx, limits, &budget, || {
        false
    })
    .unwrap();
    let same = decode_pages(&DecodeRequest::new(&path), PageKind::Dcx, limits, &budget, || {
        false
    })
    .unwrap();
    assert_eq!(first.page_count(), 2);
    assert!(first.same_pages(&same, || false).unwrap());
    assert!(
        decode_pages(
            &DecodeRequest::new(&path),
            PageKind::Dcx,
            AnimationBudget {
                max_frames: 1,
                ..limits
            },
            &budget,
            || false
        )
        .is_err()
    );
    assert!(
        decode_pages(
            &DecodeRequest::new(root.join("gif-animation-disposal-1.gif")),
            PageKind::Dcx,
            limits,
            &budget,
            || false
        )
        .is_err()
    );
    let mut bytes = std::fs::read(&path).unwrap();
    // DCX table gives the second page offset; PCX 8-bit palette occupies final 768 bytes.
    let second_offset = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert!(second_offset < bytes.len() - 768);
    let palette = bytes.len() - 768;
    bytes[palette..].fill(0);
    let dir = tempfile::tempdir().unwrap();
    let changed = dir.path().join("changed.dcx");
    std::fs::write(&changed, bytes).unwrap();
    let changed = decode_pages(
        &DecodeRequest::new(changed),
        PageKind::Dcx,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(!first.same_pages(&changed, || false).unwrap());
}

#[test]
fn untagged_tiff_requires_explicit_color_transform() {
    use rrrah_dedup::animated::AnimationError;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    {
        let (name, kind) = ("pattern.tif", PageKind::Tiff);
        assert!(matches!(
            decode_pages(
                &DecodeRequest::new(root.join(name)),
                kind,
                limits,
                &budget,
                || false
            ),
            Err(AnimationError::Decode(_))
        ));
    }
}

#[test]
fn every_icon_resource_is_compared_including_the_last() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let png = std::fs::read(root.join("pattern.ico")).unwrap();
    let bitmap = std::fs::read(root.join("pattern.bitmap.ico")).unwrap();
    let pack = |changed: bool| {
        let mut result = vec![0; 38];
        result[..6].copy_from_slice(&[0, 0, 1, 0, 2, 0]);
        for (i, original) in [&png, &bitmap].into_iter().enumerate() {
            let offset = u32::from_le_bytes(original[18..22].try_into().unwrap()) as usize;
            let mut payload = original[offset..].to_vec();
            if changed && i == 1 {
                for pixel in payload[40..40 + 16 * 16 * 4].as_chunks_mut::<4>().0 {
                    pixel[..3].fill(0);
                }
            }
            let entry = 6 + i * 16;
            result[entry..entry + 16].copy_from_slice(&original[6..22]);
            result[entry + 8..entry + 12]
                .copy_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
            let at = u32::try_from(result.len()).unwrap();
            result[entry + 12..entry + 16].copy_from_slice(&at.to_le_bytes());
            result.extend_from_slice(&payload);
        }
        result
    };
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.ico");
    let b = dir.path().join("b.ico");
    std::fs::write(&a, pack(false)).unwrap();
    std::fs::write(&b, pack(true)).unwrap();
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let first = decode_pages(&DecodeRequest::new(&a), PageKind::Icon, limits, &budget, || false).unwrap();
    let same = decode_pages(&DecodeRequest::new(&a), PageKind::Icon, limits, &budget, || false).unwrap();
    let changed = decode_pages(&DecodeRequest::new(&b), PageKind::Icon, limits, &budget, || false).unwrap();
    assert_eq!(first.page_count(), 2);
    assert!(first.same_pages(&same, || false).unwrap());
    assert!(!first.same_pages(&changed, || false).unwrap());
}

#[test]
fn cursor_hotspot_and_invalid_resource_offsets_are_not_ignored() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let original = root.join("pattern.ico.cur");
    let mut bytes = std::fs::read(&original).unwrap();
    let x = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
    bytes[10..12].copy_from_slice(&((x + 1) % 16).to_le_bytes());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("changed.cur");
    std::fs::write(&path, &bytes).unwrap();
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let a = decode_pages(
        &DecodeRequest::new(original),
        PageKind::Icon,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    let b = decode_pages(
        &DecodeRequest::new(&path),
        PageKind::Icon,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(!a.same_pages(&b, || false).unwrap());
    bytes[18..22].copy_from_slice(&1_u32.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    assert!(
        decode_pages(
            &DecodeRequest::new(&path),
            PageKind::Icon,
            limits,
            &budget,
            || false
        )
        .is_err()
    );
}

#[test]
fn complete_tiff_pages_with_icc_detect_last_page_changes_and_cycles() {
    use rrrah_core::RasterColorSpace;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let image = rrrah_decode::decode_raster(&DecodeRequest::new(root.join("pattern-rgba-icc.jp2"))).unwrap();
    let RasterColorSpace::Icc(profile) = image.color_space() else {
        panic!("fixture must provide qualified ICC")
    };
    let build = |changed| {
        let pixel_at = 290 + profile.len();
        let mut bytes = vec![0; pixel_at + 6];
        bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 8, 0, 0, 0]);
        bytes[284..290].copy_from_slice(&[8, 0, 8, 0, 8, 0]);
        bytes[290..pixel_at].copy_from_slice(profile);
        for page in 0..2 {
            let at = 8 + page * 138;
            bytes[at..at + 2].copy_from_slice(&11_u16.to_le_bytes());
            let tags = [
                (256_u16, 4_u16, 1_u32, 1_u32),
                (257, 4, 1, 1),
                (258, 3, 3, 284),
                (259, 3, 1, 1),
                (262, 3, 1, 2),
                (273, 4, 1, u32::try_from(pixel_at + page * 3).unwrap()),
                (277, 3, 1, 3),
                (278, 4, 1, 1),
                (279, 4, 1, 3),
                (284, 3, 1, 1),
                (34675, 7, u32::try_from(profile.len()).unwrap(), 290),
            ];
            for (index, (tag, ty, count, value)) in tags.into_iter().enumerate() {
                let entry = at + 2 + index * 12;
                bytes[entry..entry + 2].copy_from_slice(&tag.to_le_bytes());
                bytes[entry + 2..entry + 4].copy_from_slice(&ty.to_le_bytes());
                bytes[entry + 4..entry + 8].copy_from_slice(&count.to_le_bytes());
                bytes[entry + 8..entry + 12].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes[142..146].copy_from_slice(&146_u32.to_le_bytes());
        bytes[pixel_at..pixel_at + 3].copy_from_slice(&[255, 0, 0]);
        bytes[pixel_at + 3..].copy_from_slice(if changed { &[0, 255, 0] } else { &[255, 0, 0] });
        bytes
    };
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.tif");
    let b = dir.path().join("b.tif");
    std::fs::write(&a, build(false)).unwrap();
    std::fs::write(&b, build(true)).unwrap();
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let first = decode_pages(&DecodeRequest::new(&a), PageKind::Tiff, limits, &budget, || false).unwrap();
    let same = decode_pages(&DecodeRequest::new(&a), PageKind::Tiff, limits, &budget, || false).unwrap();
    let changed = decode_pages(&DecodeRequest::new(&b), PageKind::Tiff, limits, &budget, || false).unwrap();
    assert_eq!(first.page_count(), 2);
    assert!(first.same_pages(&same, || false).unwrap());
    assert!(!first.same_pages(&changed, || false).unwrap());
    let mut cycle = build(false);
    cycle[280..284].copy_from_slice(&8_u32.to_le_bytes());
    std::fs::write(&b, cycle).unwrap();
    assert!(decode_pages(&DecodeRequest::new(&b), PageKind::Tiff, limits, &budget, || false).is_err());
}

#[test]
fn independently_encoded_tiff_endian_bigtiff_and_compression_matrix() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let baseline = decode_pages(
        &DecodeRequest::new(root.join("classic-little-none-same.tif")),
        PageKind::Tiff,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    for layout in ["classic", "bigtiff"] {
        for endian in ["little", "big"] {
            for compression in ["none", "deflate", "lzw", "packbits"] {
                for changed in [false, true] {
                    let name = format!(
                        "{layout}-{endian}-{compression}-{}.tif",
                        if changed { "changed" } else { "same" }
                    );
                    let pages = decode_pages(
                        &DecodeRequest::new(root.join(&name)),
                        PageKind::Tiff,
                        limits,
                        &budget,
                        || false,
                    )
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                    assert_eq!(pages.page_count(), 3, "{name}");
                    assert_eq!(baseline.same_pages(&pages, || false).unwrap(), !changed, "{name}");
                }
            }
        }
    }
}

#[test]
fn independent_tiff_precision_and_orientation_are_preserved() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 1000,
        max_file_bytes: 100_000,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let load = |name: &str| {
        decode_pages(
            &DecodeRequest::new(root.join(name)),
            PageKind::Tiff,
            limits,
            &budget,
            || false,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let reference = load("classic-little-none-same.tif");
    for orientation in 1..=8 {
        let name = format!("orientation-{orientation}.tif");
        assert!(reference.same_pages(&load(&name), || false).unwrap(), "{name}");
    }
    for dtype in ["uint16", "float32"] {
        let reference = load(&format!("precision-{dtype}-little-same.tif"));
        for endian in ["little", "big"] {
            let same = format!("precision-{dtype}-{endian}-same.tif");
            let changed = format!("precision-{dtype}-{endian}-changed.tif");
            assert!(reference.same_pages(&load(&same), || false).unwrap(), "{same}");
            assert!(
                !reference.same_pages(&load(&changed), || false).unwrap(),
                "{changed}"
            );
        }
    }
}

#[test]
fn selected_tiff_pages_use_their_own_profile_and_keep_complete_count() {
    use rrrah_dedup::pages::decode_tiff_selected;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    for ordinal in 0..3 {
        let mut a = DecodeRequest::new(root.join("classic-little-none-same.tif"));
        a.image_index = ordinal;
        let mut b = DecodeRequest::new(root.join("bigtiff-big-lzw-changed.tif"));
        b.image_index = ordinal;
        let x = decode_tiff_selected(&a, limits, &budget, || false).unwrap();
        let y = decode_tiff_selected(&b, limits, &budget, || false).unwrap();
        assert_eq!((x.image_index(), x.image_count()), (ordinal, 3));
        assert_eq!(x.same_selected_frame(&y, || false).unwrap(), ordinal != 2);
    }
    let mut invalid = DecodeRequest::new(root.join("classic-little-none-same.tif"));
    invalid.image_index = 3;
    assert!(decode_tiff_selected(&invalid, limits, &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
}

#[test]
fn selected_tiff_budget_refusals_release_metadata_and_allow_retry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let limits = AnimationBudget {
        max_frames: 10,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    for name in ["classic-little-none-same.tif", "bigtiff-big-lzw-changed.tif"] {
        let request = DecodeRequest::new(root.join(name));
        let mut failures = 0;
        let mut successes = 0;
        for bytes in [0, 16, 64, 256, 1024, 4096, 4 * 1024 * 1024] {
            let budget = MemoryBudget::new(bytes);
            let result = rrrah_dedup::pages::decode_tiff_selected(&request, limits, &budget, || false);
            if result.is_ok() {
                successes += 1;
            } else {
                failures += 1;
            }
            drop(result);
            assert_eq!(budget.used(), 0, "{name}: {bytes}");
            assert!(budget.peak() <= bytes, "{name}: {bytes}");
        }
        assert!(failures > 0 && successes > 0, "{name}");
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let retry = rrrah_dedup::pages::decode_tiff_selected(&request, limits, &budget, || false).unwrap();
        assert_eq!(retry.image_count(), 3);
        drop(retry);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn public_selected_frame_honors_tiff_page_profiles_metadata_and_explicit_limits() {
    use rrrah_dedup::decode::{decode_selected_frame, decode_selected_frame_bounded};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiff");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 3,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    for name in [
        "classic-little-none-same.tif",
        "classic-big-lzw-same.tif",
        "bigtiff-little-packbits-same.tif",
        "bigtiff-big-deflate-same.tif",
    ] {
        for ordinal in 0..3 {
            let mut request = DecodeRequest::new(root.join(name));
            request.image_index = ordinal;
            let direct = decode_selected_frame(&request, 100, &budget, || false).unwrap();
            let explicit = decode_selected_frame_bounded(&request, limits, &budget, || false).unwrap();
            let selected =
                rrrah_dedup::pages::decode_tiff_selected(&request, limits, &budget, || false).unwrap();
            assert_eq!((direct.image_index(), direct.image_count()), (ordinal, 3));
            assert!(direct.same_selected_frame(&selected, || false).unwrap());
            assert!(direct.same_selected_frame(&explicit, || false).unwrap());
            let mut changed = DecodeRequest::new(root.join("bigtiff-big-lzw-changed.tif"));
            changed.image_index = ordinal;
            let negative = decode_selected_frame(&changed, 100, &budget, || false).unwrap();
            assert_eq!(
                direct.same_selected_frame(&negative, || false).unwrap(),
                ordinal != 2
            );
        }
    }
    let request = DecodeRequest::new(root.join("classic-little-none-same.tif"));
    for denied in [
        AnimationBudget {
            max_frames: 2,
            ..limits
        },
        AnimationBudget {
            max_pixels: 1,
            ..limits
        },
        AnimationBudget {
            max_file_bytes: 4,
            ..limits
        },
    ] {
        assert!(decode_selected_frame_bounded(&request, denied, &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
    }
    let folder = tempfile::tempdir().unwrap();
    let renamed = folder.path().join("renamed.png");
    std::fs::copy(&request.path, &renamed).unwrap();
    let reference = decode_selected_frame(&request, 100, &budget, || false).unwrap();
    let renamed = decode_selected_frame(&DecodeRequest::new(renamed), 100, &budget, || false).unwrap();
    assert!(reference.same_selected_frame(&renamed, || false).unwrap());
    drop((reference, renamed));
    assert_eq!(budget.used(), 0);
}

#[test]
fn identical_tiff_samples_with_different_page_profiles_produce_distinct_colors() {
    use rrrah_dedup::decode::{decode_selected_frame, decode_selected_frame_bounded};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/page-color");
    let budget = MemoryBudget::new(4 * 1024 * 1024);
    let limits = AnimationBudget {
        max_frames: 2,
        max_pixels: 100,
        max_file_bytes: 100_000,
    };
    let srgb =
        decode_selected_frame(&DecodeRequest::new(root.join("srgb.png")), 100, &budget, || false).unwrap();
    let linear = decode_selected_frame(&DecodeRequest::new(root.join("linear.png")), 100, &budget, || {
        false
    })
    .unwrap();
    assert!(!srgb.same_selected_frame(&linear, || false).unwrap());
    for (name, actual) in [("srgb", &srgb), ("linear", &linear)] {
        let oracle = std::fs::read(root.join(format!("{name}.rgba32le"))).unwrap();
        assert_eq!(oracle.len(), 20 * 16);
        let oracle = oracle
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect::<Vec<_>>();
        let oracle = rrrah_dedup::linear::LinearRgbaView::new(5, 4, &oracle, 20, || false).unwrap();
        let evidence = rrrah_dedup::warp::verify_pixels(
            &oracle,
            &actual.view(|| false).unwrap(),
            rrrah_dedup::geometry::Transform {
                a: 1.0,
                b: 0.0,
                translation: [0.; 2],
            },
            rrrah_dedup::warp::WarpPolicy {
                // Independent ICC engines/profile matrix quantization need a declared
                // numerical color tolerance; this does not change exact pixel equality.
                tolerance: 0.0005,
                max_source_pixels: 20,
            },
            || false,
        )
        .unwrap();
        println!(
            "{name}: maximum linear channel error {}",
            evidence.maximum_channel_error
        );
        assert_eq!(
            (evidence.compared_pixels, evidence.matched_pixels),
            (20, 20),
            "{name}: {evidence:?}"
        );
    }

    for (ordinal, reference) in [(0, &srgb), (1, &linear)] {
        let mut request = DecodeRequest::new(root.join("mixed.tif"));
        request.image_index = ordinal;
        let direct = decode_selected_frame(&request, 100, &budget, || false).unwrap();
        assert_eq!((direct.image_index(), direct.image_count()), (ordinal, 2));
        assert!(direct.same_selected_frame(reference, || false).unwrap());
        request.assume_untagged_srgb = true;
        let explicit = decode_selected_frame_bounded(&request, limits, &budget, || false).unwrap();
        assert!(explicit.same_selected_frame(reference, || false).unwrap());
        let mut changed_profile = DecodeRequest::new(root.join("all-srgb.tif"));
        changed_profile.image_index = ordinal;
        let changed = decode_selected_frame(&changed_profile, 100, &budget, || false).unwrap();
        assert_eq!(
            direct.same_selected_frame(&changed, || false).unwrap(),
            ordinal == 0
        );
    }
    let mixed = decode_pages(
        &DecodeRequest::new(root.join("mixed.tif")),
        PageKind::Tiff,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    let all_srgb = decode_pages(
        &DecodeRequest::new(root.join("all-srgb.tif")),
        PageKind::Tiff,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(!mixed.same_pages(&all_srgb, || false).unwrap());
    drop((srgb, linear, mixed, all_srgb));
    assert_eq!(budget.used(), 0);
}
