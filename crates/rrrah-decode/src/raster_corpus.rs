//! Independent-codec baseline fixtures. Oracle files never come from Rrrah.
use crate::{DecodedImage, decode_image_file};
use rrrah_core::RasterPixels;
use std::path::Path;

#[test]
fn independent_raster_corpus_matches_dimensions_alpha_and_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let manifest = include_str!("../../../tests/fixtures/raster/manifest.tsv");
    let mut count = 0;
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6);
        let path = root.join(fields[0]);
        let width: u32 = fields[1].parse().unwrap();
        let height: u32 = fields[2].parse().unwrap();
        let tolerance: u8 = fields[3].parse().unwrap();
        let reference = std::fs::read(root.join(fields[4])).unwrap();
        let result = decode_image_file(&path).unwrap_or_else(|error| panic!("{}: {error}", fields[0]));
        let DecodedImage::Raster(frame) = result else {
            panic!("raster routed to sensor: {}", fields[0])
        };
        assert_eq!((frame.width(), frame.height()), (width, height), "{}", fields[0]);
        let RasterPixels::Rgba8(pixels) = frame.pixels() else {
            panic!("unexpected sample type: {}", fields[0])
        };
        assert_eq!(pixels.len(), reference.len());
        for (index, (&actual, &expected)) in pixels.iter().zip(&reference).enumerate() {
            assert!(
                actual.abs_diff(expected) <= tolerance,
                "{} sample {index}: {actual} vs {expected}, tolerance {tolerance}",
                fields[0]
            );
        }
        let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
        let mut request = crate::DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let managed =
            crate::decode_raster(&request).unwrap_or_else(|error| panic!("managed {}: {error}", fields[0]));
        assert_eq!((managed.width(), managed.height()), (width, height));
        assert_eq!(managed.color_space(), frame.color_space(), "{}", fields[0]);
        let RasterPixels::Rgba8(retained) = managed.pixels() else {
            panic!("managed sample type changed: {}", fields[0]);
        };
        assert!(retained.is_managed(), "{}", fields[0]);
        assert_eq!(retained.as_slice(), pixels.as_slice(), "{}", fields[0]);
        let pixel_capacity = managed.pixel_capacity_bytes();
        let capacity = managed.capacity_bytes();
        assert_eq!(budget.used(), capacity, "{}", fields[0]);
        let owner = retained.clone();
        drop(managed);
        assert_eq!(budget.used(), pixel_capacity, "{}", fields[0]);
        drop(owner);
        assert_eq!(budget.used(), 0, "{}", fields[0]);
        count += 1;
    }
    assert_eq!(count, 150, "corpus coverage changed; inspect the manifest");
}

#[test]
fn each_independent_container_rejects_a_truncated_header() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for line in include_str!("../../../tests/fixtures/raster/manifest.tsv").lines() {
        let name = line.split('\t').next().unwrap();
        let bytes = std::fs::read(root.join(name)).unwrap();
        let path = std::env::temp_dir().join(format!("rrrah-truncated-{}-{name}", std::process::id()));
        std::fs::write(&path, &bytes[..8]).unwrap();
        let result = decode_image_file(&path);
        std::fs::remove_file(path).unwrap();
        assert!(result.is_err(), "truncated {name} unexpectedly decoded");
    }
}

#[test]
fn profiled_avif_reaches_linear_display_and_preserves_alpha() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let DecodedImage::Raster(frame) = decode_image_file(root.join("avif-alpha-icc.avif")).unwrap() else {
        panic!()
    };
    assert!(matches!(
        frame.color_space(),
        rrrah_core::RasterColorSpace::Icc(_)
    ));
    let prepared = crate::prepare_raster_for_display(&frame).unwrap();
    let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
        panic!()
    };
    let oracle = std::fs::read(root.join("avif-alpha-icc.avif.rgba")).unwrap();
    for (actual, reference) in p.chunks_exact(4).zip(oracle.chunks_exact(4)) {
        assert!((actual[3] - f32::from(reference[3]) / 255.0).abs() < 1e-7);
        assert!(actual[..3].iter().all(|v| v.is_finite()));
    }
}

#[test]
fn nclx_srgb_avif_reaches_display_without_icc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let DecodedImage::Raster(frame) = decode_image_file(root.join("avif-rgb.avif")).unwrap() else {
        panic!()
    };
    assert_eq!(frame.color_space(), &rrrah_core::RasterColorSpace::Srgb);
    assert!(crate::prepare_raster_for_display(&frame).is_ok());
}

#[test]
fn cursor_hotspot_survives_decode_and_display_transform() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for name in ["pattern.ico.cur", "pattern.bitmap.ico.cur"] {
        let DecodedImage::Raster(frame) = decode_image_file(root.join(name)).unwrap() else {
            panic!()
        };
        assert_eq!(frame.hotspot(), Some((3, 5)));
        assert_eq!(
            crate::prepare_raster_for_display(&frame).unwrap().hotspot(),
            Some((3, 5))
        );
        assert!(frame.with_hotspot(Some((16, 0))).is_err());
    }
}

#[test]
fn openraster_mime_magic_overrides_camera_suffix() {
    let bytes = include_bytes!("../../../tests/fixtures/raster/pattern.png.ora");
    let path = std::env::temp_dir().join(format!("rrrah-ora-renamed-{}.cr3", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let result = decode_image_file(&path);
    std::fs::remove_file(path).unwrap();
    assert!(matches!(result, Ok(DecodedImage::Raster(_))), "{result:?}");
}

#[test]
fn independent_precision_corpus_preserves_every_native_sample() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let manifest = include_str!("../../../tests/fixtures/raster/precision-manifest.tsv");
    let mut count = 0;
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6);
        let (width, height) = (
            fields[1].parse::<u32>().unwrap(),
            fields[2].parse::<u32>().unwrap(),
        );
        let oracle = std::fs::read(root.join(fields[4])).unwrap();
        let DecodedImage::Raster(frame) =
            decode_image_file(root.join(fields[0])).unwrap_or_else(|error| panic!("{}: {error}", fields[0]))
        else {
            panic!("precision raster routed to sensor")
        };
        assert_eq!((frame.width(), frame.height()), (width, height), "{}", fields[0]);
        match (fields[3], frame.pixels()) {
            ("u8", RasterPixels::Rgba8(pixels)) => {
                assert_eq!(pixels.as_slice(), oracle.as_slice(), "{}", fields[0])
            }
            ("u16", RasterPixels::Rgba16(pixels)) => {
                assert_eq!(pixels.len() * 2, oracle.len());
                for (index, (actual, expected)) in pixels.iter().zip(oracle.chunks_exact(2)).enumerate() {
                    assert_eq!(
                        *actual,
                        u16::from_le_bytes(expected.try_into().unwrap()),
                        "{} sample {index}",
                        fields[0]
                    );
                }
            }
            ("f32", RasterPixels::Rgba32Float(pixels)) => {
                assert_eq!(pixels.len() * 4, oracle.len());
                for (index, (actual, expected)) in pixels.iter().zip(oracle.chunks_exact(4)).enumerate() {
                    assert_eq!(
                        actual.to_bits(),
                        f32::from_le_bytes(expected.try_into().unwrap()).to_bits(),
                        "{} sample {index}",
                        fields[0]
                    );
                }
            }
            _ => panic!("{}: sample precision/type changed", fields[0]),
        }
        count += 1;
    }
    assert_eq!(count, 16, "precision coverage changed; inspect the manifest");
}

#[test]
fn precision_containers_reject_truncated_headers_and_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for line in include_str!("../../../tests/fixtures/raster/precision-manifest.tsv").lines() {
        let name = line.split('\t').next().unwrap();
        let bytes = std::fs::read(root.join(name)).unwrap();
        for length in [8, bytes.len() / 2] {
            let path = std::env::temp_dir().join(format!(
                "rrrah-precision-truncated-{}-{length}-{name}",
                std::process::id()
            ));
            std::fs::write(&path, &bytes[..length]).unwrap();
            let result = decode_image_file(&path);
            std::fs::remove_file(path).unwrap();
            assert!(
                result.is_err(),
                "truncated {name} ({length}) unexpectedly decoded"
            );
        }
    }
}

#[test]
fn farbfeld_interoperability_color_and_straight_alpha_reach_display() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let DecodedImage::Raster(frame) = decode_image_file(root.join("adjacent16.ff")).unwrap() else {
        panic!()
    };
    assert_eq!(frame.color_space(), &rrrah_core::RasterColorSpace::AssumedSrgb);
    let prepared = crate::prepare_raster_for_display(&frame).unwrap();
    let RasterPixels::Rgba32Float(p) = prepared.pixels() else {
        panic!()
    };
    let sample = 32768.0_f32 / 65535.0;
    let expected = ((sample + 0.055) / 1.055).powf(2.4);
    assert!((p[0] - expected).abs() < 1e-7);
    assert!(p[4] > p[0]);
    assert!((p[3] - 32767.0 / 65535.0).abs() < 1e-7);
    assert!((p[7] - 32768.0 / 65535.0).abs() < 1e-7);
}

#[test]
fn exr_zero_alpha_emission_is_not_silently_discarded() {
    let bytes = include_bytes!("../../../tests/fixtures/raster/zero-alpha-emission.exr");
    let result =
        crate::raster::decode_raster_bytes(bytes.to_vec(), &crate::DecodeRequest::new("emission.exr"));
    assert!(matches!(
        result,
        Err(crate::RasterDecodeError::InvalidExrAlpha(
            "zero-alpha emission requires associated-alpha rendering"
        ))
    ));
}

#[test]
fn exr_straight_alpha_matches_associated_reference_composition() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let background = [0.25_f32, 0.5, 0.75];
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/precision-manifest.tsv").lines() {
        let name = line.split('\t').next().unwrap();
        if !name.ends_with(".exr") {
            continue;
        }
        let DecodedImage::Raster(frame) = decode_image_file(root.join(name)).unwrap() else {
            panic!()
        };
        let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
            panic!()
        };
        let oracle = std::fs::read(root.join(format!("{name}.over-rgb-f32"))).unwrap();
        assert_eq!(oracle.len(), pixels.len() / 4 * 3 * 4);
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            for channel in 0..3 {
                let at = (index * 3 + channel) * 4;
                let expected = f32::from_le_bytes(oracle[at..at + 4].try_into().unwrap());
                let actual = pixel[channel] * pixel[3] + (1.0 - pixel[3]) * background[channel];
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "{name} pixel {index} channel {channel}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn macbinary_magic_wins_over_a_misleading_raw_extension() {
    let bytes = include_bytes!("../../../tests/fixtures/raster/wrapped-130-1.mac");
    let path = std::env::temp_dir().join(format!("rrrah-macbinary-{}.cr3", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let result = decode_image_file(&path);
    std::fs::remove_file(&path).unwrap();
    let DecodedImage::Raster(frame) = result.unwrap() else {
        panic!("wrapper routed to sensor")
    };
    assert_eq!((frame.width(), frame.height()), (576, 720));
}

#[test]
fn wal_with_explicit_palette_matches_pillow_and_preserves_transparent_rgb() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let bytes = include_bytes!("../../../tests/fixtures/raster/wal-palette.rgba");
    let mut palette = [[0u8; 4]; 256];
    for (pixel, bytes) in palette.iter_mut().zip(bytes.chunks_exact(4)) {
        pixel.copy_from_slice(bytes);
    }
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/wal-manifest.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5);
        let path = root.join(fields[0]);
        let request = crate::DecodeRequest::new(&path);
        let frame =
            crate::decode_wal_with_palette(&request, &palette, rrrah_core::RasterColorSpace::Srgb).unwrap();
        assert_eq!(
            (frame.width(), frame.height()),
            (fields[1].parse().unwrap(), fields[2].parse().unwrap())
        );
        let RasterPixels::Rgba8(pixels) = frame.pixels() else {
            panic!()
        };
        assert_eq!(
            pixels.as_slice(),
            std::fs::read(root.join(fields[3])).unwrap().as_slice()
        );
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[3] == 0 && p[..3].iter().any(|v| *v != 0))
        );
        crate::prepare_raster_for_display(&frame).unwrap();
        assert!(matches!(
            decode_image_file(&path),
            Err(crate::RasterDecodeError::WalPaletteRequired)
        ));
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn wal_game_tree_resolves_palette_through_public_router() {
    let dir = std::env::temp_dir().join(format!("rrrah-wal-game-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("textures/test")).unwrap();
    std::fs::create_dir(dir.join("pics")).unwrap();
    let path = dir.join("textures/test/grid.wal");
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
    )
    .unwrap();
    assert!(matches!(
        decode_image_file(&path),
        Err(crate::RasterDecodeError::WalPaletteRequired)
    ));
    let palette_path = dir.join("pics/colormap.pcx");
    std::fs::write(
        &palette_path,
        include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
    )
    .unwrap();
    let DecodedImage::Raster(frame) = decode_image_file(&path).unwrap() else {
        panic!()
    };
    let RasterPixels::Rgba8(pixels) = frame.pixels() else {
        panic!()
    };
    assert_eq!(
        pixels.as_slice(),
        include_bytes!("../../../tests/fixtures/raster/palette-grid.wal.rgba")
    );
    crate::prepare_raster_for_display(&frame).unwrap();
    std::fs::write(&palette_path, b"invalid palette").unwrap();
    assert!(decode_image_file(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dpx_storage_matches_independent_ffmpeg_samples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/dpx-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let DecodedImage::Raster(frame) = decode_image_file(root.join(f[0])).unwrap() else {
            panic!()
        };
        assert_eq!(
            (frame.width(), frame.height()),
            (f[1].parse().unwrap(), f[2].parse().unwrap())
        );
        let oracle = std::fs::read(root.join(f[4])).unwrap();
        match frame.pixels() {
            RasterPixels::Rgba8(p) => assert_eq!(p.as_slice(), oracle.as_slice(), "{}", f[0]),
            RasterPixels::Rgba16(p) => assert_eq!(
                p.as_slice(),
                oracle
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>(),
                "{}",
                f[0]
            ),
            _ => panic!(),
        }
        count += 1;
    }
    assert_eq!(count, 24);
}

#[test]
fn dpx_signature_overrides_camera_suffix() {
    let path = std::env::temp_dir().join(format!("rrrah-dpx-{}.cr3", std::process::id()));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/dpx-10-be-50-p1.dpx"),
    )
    .unwrap();
    let result = decode_image_file(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(result, Ok(DecodedImage::Raster(_))));
    assert!(crate::is_supported_image_path(Path::new("image.DPX")));
}

#[test]
fn cineon_samples_match_openimageio_and_camera_suffix_routes_raster() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/cineon-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let DecodedImage::Raster(frame) = decode_image_file(root.join(f[0])).unwrap() else {
            panic!()
        };
        assert_eq!((frame.width(), frame.height()), (5, 3));
        let oracle = std::fs::read(root.join(f[4])).unwrap();
        match frame.pixels() {
            RasterPixels::Rgba8(p) => assert_eq!(p.as_slice(), oracle.as_slice(), "{}", f[0]),
            RasterPixels::Rgba16(p) => assert_eq!(
                p.as_slice(),
                oracle
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>(),
                "{}",
                f[0]
            ),
            _ => panic!(),
        }
        assert!(crate::prepare_raster_for_display(&frame).is_err());
        count += 1;
    }
    assert_eq!(count, 12);
    let path = std::env::temp_dir().join(format!("rrrah-cineon-{}.cr3", std::process::id()));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/cineon-10-be-3-p5.cin"),
    )
    .unwrap();
    let result = decode_image_file(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(result, Ok(DecodedImage::Raster(_))));
    assert!(crate::is_supported_image_path(Path::new("frame.CIN")));
}

#[test]
fn softimage_packets_match_openimageio_native_samples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/softimage-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let DecodedImage::Raster(frame) = decode_image_file(root.join(f[0])).unwrap() else {
            panic!()
        };
        assert_eq!((frame.width(), frame.height()), (140, 3));
        let oracle = std::fs::read(root.join(f[4])).unwrap();
        match frame.pixels() {
            RasterPixels::Rgba8(p) => assert_eq!(p.as_slice(), oracle.as_slice(), "{}", f[0]),
            RasterPixels::Rgba16(p) => assert_eq!(
                p.as_slice(),
                oracle
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>(),
                "{}",
                f[0]
            ),
            _ => panic!(),
        };
        crate::prepare_raster_for_display(&frame).unwrap();
        count += 1;
    }
    assert_eq!(count, 12);
}

#[test]
fn softimage_signature_overrides_raw_extension() {
    let path = std::env::temp_dir().join(format!("rrrah-softimage-{}.cr3", std::process::id()));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/softimage-16-a16-rle2.pic"),
    )
    .unwrap();
    let result = decode_image_file(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(result, Ok(DecodedImage::Raster(_))));
    assert!(crate::is_supported_image_path(Path::new("image.PIC")));
}

#[test]
fn rla_indexed_rows_and_byte_planes_match_openimageio() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/rla-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let path = root.join(f[0]);
        let frame = crate::decode_rla_with_interpretation(
            &crate::DecodeRequest::new(&path),
            crate::RlaAlphaMode::Straight,
            rrrah_core::RasterColorSpace::Unspecified,
        )
        .unwrap();
        if !f[0].contains("-a0-") {
            assert!(matches!(
                decode_image_file(&path),
                Err(crate::RasterDecodeError::RlaInterpretationRequired)
            ));
        }
        assert_eq!((frame.width(), frame.height()), (140, 3));
        let oracle = std::fs::read(root.join(f[4])).unwrap();
        match frame.pixels() {
            RasterPixels::Rgba8(p) => assert_eq!(p.as_slice(), oracle.as_slice(), "{}", f[0]),
            RasterPixels::Rgba16(p) => assert_eq!(
                p.as_slice(),
                oracle
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>(),
                "{}",
                f[0]
            ),
            _ => panic!(),
        };
        assert!(crate::prepare_raster_for_display(&frame).is_err());
        count += 1;
    }
    assert_eq!(count, 14);
    assert!(crate::is_supported_image_path(Path::new("image.RLA")));
}

#[test]
fn rla_premultiplied_interpretation_preserves_compositing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let request = crate::DecodeRequest::new(root.join("rla-16-c3-a16-mixed0.rla"));
    let frame = crate::decode_rla_with_interpretation(
        &request,
        crate::RlaAlphaMode::Premultiplied,
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let RasterPixels::Rgba32Float(pixels) = frame.pixels() else {
        panic!()
    };
    let oracle = std::fs::read(root.join("rla-16-c3-a16-mixed0.rla.u16")).unwrap();
    let values: Vec<_> = oracle
        .chunks_exact(2)
        .map(|v| f32::from(u16::from_le_bytes(v.try_into().unwrap())) / 65535.0)
        .collect();
    for (actual, stored) in pixels.chunks_exact(4).zip(values.chunks_exact(4)) {
        for (c, bg) in [0.25, 0.5, 0.75].into_iter().enumerate() {
            let composition = actual[c] * actual[3] + bg * (1.0 - actual[3]);
            let expected = stored[c] + bg * (1.0 - stored[3]);
            assert!((composition - expected).abs() < 2e-7);
        }
    }
    crate::prepare_raster_for_display(&frame).unwrap();
}

#[test]
fn wad3_selected_textures_match_vgio_indices_and_authored_palette() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut count = 0;
    for line in include_str!("../../../tests/fixtures/raster/wad-manifest.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let mut request = crate::DecodeRequest::new(root.join(f[0]));
        request.image_index = f[1].parse().unwrap();
        let crate::DecodedImage::Raster(frame) = crate::decode_image(&request).unwrap() else {
            panic!()
        };
        assert_eq!(
            (frame.width(), frame.height()),
            (f[2].parse().unwrap(), f[3].parse().unwrap())
        );
        let RasterPixels::Rgba8(p) = frame.pixels() else {
            panic!()
        };
        assert_eq!(p.as_slice(), std::fs::read(root.join(f[4])).unwrap().as_slice());
        assert_eq!(p.chunks_exact(4).any(|v| v[3] == 0), request.image_index == 1);
        crate::prepare_raster_for_display(&frame).unwrap();
        count += 1;
    }
    assert_eq!(count, 4);
    let path = std::env::temp_dir().join(format!("rrrah-wad-{}.cr3", std::process::id()));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/wad3-textures.wad"),
    )
    .unwrap();
    let mut request = crate::DecodeRequest::new(&path);
    request.image_index = 1;
    let result = crate::decode_image(&request);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(result, Ok(crate::DecodedImage::Raster(_))));
    assert!(crate::is_supported_image_path(Path::new("images.WAD")));
}

#[test]
fn dcx_selection_under_camera_suffix_preserves_page_metadata() {
    let path = std::env::temp_dir().join(format!("rrrah-dcx-selected-{}.cr3", std::process::id()));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/raster/two-pages.dcx"),
    )
    .unwrap();
    let mut request = crate::DecodeRequest::new(&path);
    request.image_index = 1;
    let result = crate::decode_image(&request);
    std::fs::remove_file(&path).unwrap();
    let crate::DecodedImage::Raster(frame) = result.unwrap() else {
        panic!()
    };
    assert_eq!((frame.image_index(), frame.image_count()), (1, 2));
    let RasterPixels::Rgba8(p) = frame.pixels() else {
        panic!()
    };
    assert_eq!(
        p.as_slice(),
        include_bytes!("../../../tests/fixtures/raster/two-pages.dcx.page1.rgba")
    );
    let prepared = crate::prepare_raster_for_display(&frame).unwrap();
    assert_eq!((prepared.image_index(), prepared.image_count()), (1, 2));
}
